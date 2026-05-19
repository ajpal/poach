use std::fmt::{self, Display, Formatter};
use std::time::{Duration, Instant};

use serde::{Serialize, Serializer};

type TimerHandle = usize;

#[derive(Default)]
pub struct Reporter {
    timers: Vec<Timer>,
    sizes: Vec<Size>,
}

#[derive(Clone, Serialize)]
pub struct Timer {
    name: String,
    steps: Vec<TimingStep>,

    #[serde(skip)]
    started_at: Instant,
}

#[derive(Clone, Serialize)]
struct TimingStep {
    step_name: String,
    #[serde(serialize_with = "serialize_duration_total")]
    duration: Duration,
}

#[derive(Serialize)]
pub struct RunReport {
    timers: Vec<Timer>,
    sizes: Vec<Size>,
}

#[derive(Clone, Serialize)]
pub struct Size {
    name: String,
    value: Metric,
}

#[derive(Clone, Serialize, PartialEq, Eq, Debug)]
pub enum Metric {
    Count(u64),
    Bytes(u64),
}

impl Reporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_timer(&mut self, name: &str) -> TimerHandle {
        let handle = self.timers.len();
        self.timers.push(Timer {
            name: name.to_string(),
            started_at: Instant::now(),
            steps: vec![],
        });
        handle
    }

    pub fn record_timer(&mut self, h: TimerHandle, name: &str) {
        let old = self.timers[h].started_at;
        let cur = Instant::now();
        self.timers[h].steps.push(TimingStep {
            step_name: name.to_string(),
            duration: cur.duration_since(old),
        });
        self.timers[h].started_at = cur;
    }

    pub fn record_size(&mut self, name: &str, value: Metric) {
        self.sizes.push(Size {
            name: name.to_string(),
            value,
        });
    }

    pub fn build_report(&self) -> RunReport {
        RunReport {
            timers: self.timers.clone(),
            sizes: self.sizes.clone(),
        }
    }
}

fn serialize_duration_total<S>(total: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // Clock granularity is ~50-100 ns.
    // Serialize as micros to avoid reporting false precision.
    total.as_micros().serialize(serializer)
}

impl Display for RunReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "Timers:")?;
        for t in self.timers.iter() {
            writeln!(f, "  {}", t)?;
        }

        writeln!(f, "Sizes:")?;
        for metric in &self.sizes {
            writeln!(f, "  {}", metric)?;
        }

        Ok(())
    }
}

impl Display for Timer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "Timer: {}", self.name)?;
        for step in &self.steps {
            writeln!(f, "    {}", step)?;
        }
        Ok(())
    }
}

impl Display for TimingStep {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "TimingStep: {}", self.step_name)?;
        writeln!(f, "      duration: {}", self.duration.as_micros())?;

        Ok(())
    }
}

impl Display for Size {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} = {}", self.name, self.value)
    }
}

impl Display for Metric {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Metric::Count(value) => write!(f, "{value}"),
            Metric::Bytes(value) => write!(f, "{} bytes", value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_report() {
        let mut reporter = Reporter::new();

        let t1 = reporter.new_timer("timer_1");
        reporter.record_timer(t1, "step_one");

        let t2 = reporter.new_timer("timer_2");
        std::thread::sleep(Duration::from_millis(50));
        reporter.record_timer(t2, "step_two");
        std::thread::sleep(Duration::from_millis(50));
        reporter.record_timer(t2, "step_three");

        reporter.record_size("size_one", Metric::Count(10));

        let report = reporter.build_report();

        println!("{}", report);

        assert_eq!(report.timers.len(), 2);
        assert_eq!(report.timers[0].steps.len(), 1);
        assert_eq!(report.timers[0].steps[0].step_name, "step_one");
        assert_eq!(report.timers[1].steps.len(), 2);
        assert!(report.timers[1].steps[1].duration >= Duration::from_millis(50));

        assert_eq!(report.sizes.len(), 1);
        assert_eq!(report.sizes[0].value, Metric::Count(10));
    }
}
