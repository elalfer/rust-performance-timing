use std::time::Instant;
use core::arch::x86_64::_rdtsc;
use criterion::measurement::Measurement;
use criterion::measurement::ValueFormatter;
use criterion::Throughput;

/// Run function with known latency.
/// Assume both `sub` & `and` ops are single cycle on all architectures.
/// Might not behave as expected with another code running on the same
/// phisical core.
/// 
/// Returns final counter value. Useful for creating dep chains with known latency.
pub fn const_cycle_loop(mut cycles: u64) -> u64 {
    assert_eq!(cycles % 4, 0);
    assert!(cycles < 0xFFFFFFFFu64);

    // Trick the compiler not to optimize this loop out
    cycles = cycles / 2;
    while cycles > 0 {
        cycles = cycles - 1;
        cycles = cycles & 0x700FFFFFFFFu64;
    }
    return cycles;
}

/// CPU frequency information structure
#[derive(Debug)]
pub struct FreqInfo {
    /// Current core running frequency
    frequency: f32,
    /// Time stamp counter scaling factor
    tsc_scaling: f32
}

/// CPU information structure. Provides frequency and TSC-to-cycle scaling information.
pub struct CPUInfo;

impl CPUInfo {
    /// Get current core frequency.
    /// Runs known latency loop and time it. This information allows to calculate core frequency.
    /// Current method might not work correctly when something running on second thread (SMT).
    pub fn get_frequency_hz() -> FreqInfo {
        let tot_cycles = 1_000_000;
        let start = Instant::now();
        // TODO: More accurate freq measurement
        let ts_s = CPUInfo::get_time_stamp();
        let r = const_cycle_loop(tot_cycles);
        let ts_e = CPUInfo::get_time_stamp();
        let elapsed = start.elapsed();
        let time_ns = elapsed.as_nanos() + r as u128;
        let mut freq = (1e9 * tot_cycles as f32) / ( time_ns as f32 );
        freq = (freq  / 50_000_000f32) * 50_000_000f32;

        return FreqInfo {
            frequency: freq,
            tsc_scaling: tot_cycles as f32 / (ts_e - ts_s) as f32 };
    }

    /// Get core frequency in GHz.
    /// Uses `CPUInfo::get_frequency_hz` method.
    pub fn get_frequency_ghz() -> FreqInfo {
        let mut r = CPUInfo::get_frequency_hz();
        r.frequency /= 1e9f32;
        return r;
    }

    /// Get current CPU time stamp counter value
    /// Uses `RDTSC` instruction on `x86` architectures
    pub fn get_time_stamp() -> u64 {
        let r: u64;
        unsafe {
            r = _rdtsc();
        }
      return r;
    }
}

/// Measure code block performance by collecting samples.
/// It relies on the fact that objects get destroyed 
/// at the end of the block.
/// 
/// ```rust
/// let mut loop_timing = MeasureRegion::new();
/// for _ in 0..N {
///   let _ = loop_timing.get_sample();
///   foo();
/// }
/// let cpu_cycles = loop_timing.get_average_sample() *
///   CPUInfo::get_frequency_hz().tsc_scaling;
/// ```
pub struct MeasureRegion {
    region_name: String,
    dump_on_drop: bool,
    num_samples: u64,
    sum_samples: u64
}

/// Measurement sample created by `MeasureRegion`
/// Shall not be created directly.
pub struct MeasureSample<'a> {
    parent: &'a mut MeasureRegion,
    start_time: u64,
    end_time: u64
}

impl MeasureRegion {
    pub fn new_named(region_name: String, dump_on_drop: bool) -> Self {
        MeasureRegion { region_name, dump_on_drop, num_samples: 0, sum_samples: 0 }
    }
    
    pub fn new() -> Self {
        MeasureRegion { region_name: String::from("default_name"), dump_on_drop: false,
                        num_samples: 0, sum_samples: 0 }
    }

    pub fn get_sample(&mut self) -> MeasureSample {
        MeasureSample::new(self)
    }

    pub fn get_average_sample(&self) -> f32 {
        return self.sum_samples as f32 / self.num_samples as f32;
    }

    /// Get total running time in milliseconds
    pub fn get_total_time(&self) -> u64 {
        return self.sum_samples;
    }

    fn record_sample(&mut self, sample: u64) {
        self.num_samples += 1;
        self.sum_samples += sample;
    }
}

impl Drop for MeasureRegion {
    fn drop(&mut self) {
        if self.dump_on_drop {
            println!("{}: {} ref.cycles", self.region_name, self.get_average_sample());
        }
    }
}

impl<'a> MeasureSample<'a> {

  pub fn new(parent: &'a mut MeasureRegion) -> Self {
    MeasureSample { parent, start_time: CPUInfo::get_time_stamp(), end_time: 0 }
  }

  /// Get sample value
  fn get_value(&self) -> u64 {
      self.end_time - self.start_time
  }
}

impl<'a> Drop for MeasureSample<'a> {
    fn drop(&mut self) {
        // Store sample in the parent container
        if self.end_time == 0 {
            self.end_time = CPUInfo::get_time_stamp();
        }
        self.parent.record_sample(self.get_value());
    }
}

// Get function running time in reference cycles
pub fn measure_function_perf<F>(f: F)  -> f32
where F: Fn() {
    let min_test: usize = 100;
    let min_bench_time: u64 = 10_000_000;

    let mut m = MeasureRegion::new();

    while m.get_total_time() < min_bench_time {
        let _s = m.get_sample();
        for _ in 0..min_test {
            f();
        }
    }
    return m.get_average_sample() / min_test as f32;
}

pub struct CycleInstant {
    start: u64
}

impl CycleInstant {
    pub fn now() -> CycleInstant {
        CycleInstant { start: CPUInfo::get_time_stamp() }
    }

    pub fn elapsed(&self) -> u64 {
        CPUInfo::get_time_stamp() - self.start
    }
}

/// Custom cycle accurate measurement class for criterion
/// 
/// ```rust
/// pub fn criterion_benchmark(c: &mut Criterion<CriterionCycleCounter>) {
///   c.bench_function("cycle_10K", |b| b.iter(|| const_cycle_loop(black_box(10_000))));
/// }
///
/// fn core_cycle_measurement() -> Criterion<CriterionCycleCounter> {
///   Criterion::default().with_measurement(CriterionCycleCounter)
/// }
///
/// criterion_group! {
///   name = benches;
///   config = core_cycle_measurement();
///   targets = criterion_benchmark
/// }
/// ```
pub struct CriterionCycleCounter;

impl Measurement for CriterionCycleCounter {
    type Intermediate = CycleInstant;
    type Value = u64;

    fn start(&self) -> Self::Intermediate {
        CycleInstant::now()
    }

    fn end(&self, i: Self::Intermediate) -> Self::Value {
        i.elapsed()
    }

    fn add(&self, v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        *v1 + *v2
    }

    fn zero(&self) -> Self::Value {
        0u64
    }

    fn to_f64(&self, val: &Self::Value) -> f64 {
        *val as f64 * CPUInfo::get_frequency_hz().tsc_scaling as f64
    }

    fn formatter(&self) -> &dyn ValueFormatter {
        &CriterionCycleCounter
    }
}

impl ValueFormatter for CriterionCycleCounter {
    fn format_value(&self, value: f64) -> String {
        format!("{:.3} clocks", value)
    }

    fn format_throughput(&self, throughput: &Throughput, value: f64) -> String {
        match *throughput {
            Throughput::Bytes(bytes) => format!(
                "{} b/c",
                bytes as f64 / (value)
            ),
            Throughput::Elements(elems) => format!(
                "{} elem/c",
                elems as f64 / (value)
            ),
        }
    }

    fn scale_values(&self, _typical_value: f64, _values: &mut [f64]) -> &'static str {
        "clocks"
    }

    fn scale_throughputs(&self, _typical_value: f64, throughput: &Throughput, _values: &mut [f64]) -> &'static str {
        match *throughput {
            Throughput::Bytes(_bytes) => {
                "b/c"
            }
            Throughput::Elements(_elems) => {
                "elem/c"
            }
        }
    }
    
    fn scale_for_machines(&self, _values: &mut [f64]) -> &'static str {
        "clocks"
    }
}

pub fn cycle_accurate_config() -> criterion::Criterion<CriterionCycleCounter> {
    criterion::Criterion::default().with_measurement(CriterionCycleCounter)
}

#[cfg(test)]
mod tests {
    use crate::{CPUInfo, MeasureRegion};
    use crate::const_cycle_loop;

    #[test]
    fn it_works() {
        const_cycle_loop(200_000_000); // Warmup CPU
        for _ in 0..10 {
            println!("Current CPU freq: {}GHz", CPUInfo::get_frequency_hz().frequency / 1e9f32);
        }

        let mut loop_timing = MeasureRegion::new();
        for _ in 0..10 {
            let _s = loop_timing.get_sample();
            const_cycle_loop(100_000_000);
        }
        println!("Timing info: {}",loop_timing.get_average_sample());
    }

    #[test]
    fn test_tsc_scaling() {
        let ckl_cnt = 100_000_000u64;
        const_cycle_loop(ckl_cnt); // Warmup CPU
        let mut loop_timing = MeasureRegion::new();
        for _ in 0..10 {
            let _s = loop_timing.get_sample();
            const_cycle_loop(ckl_cnt);
        }
        let cpu_info = CPUInfo::get_frequency_hz();

        let measured_cycles = loop_timing.get_average_sample() * cpu_info.tsc_scaling;
        let d = (ckl_cnt as f32 - measured_cycles).abs();

        println!("CPU info: {:?}", cpu_info);
        println!("expected {}, measured {}", ckl_cnt, measured_cycles);

        // Measured average cycles are within 5% accuracy
        let accuracy = 0.05f32;
        assert_eq!(d < (ckl_cnt as f32 * accuracy), true);
    }
}
