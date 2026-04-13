use std::fmt::{self, Display, Formatter};
use std::time::{Duration, Instant};

use hashbrown::HashMap;
use serde::Serialize;
use tracing_subscriber::Registry;

#[derive(Default)]
pub struct Reporter {
    spans: HashMap<String, SpanStats>,
    sizes: Vec<SizeMetric>,
}

#[derive(Clone, Copy, Default)]
struct SpanStats {
    count: u64,
    total: Duration,
}

#[derive(Serialize)]
pub struct RunReport {
    command: String,
    timing: TimingReport,
    sizes: Vec<SizeMetric>,
}

#[derive(Serialize)]
pub struct TimingReport {
    #[serde(with = "serde_millis")]
    total: Duration,
    // Each entry is (step name, invocation count, elapsed time).
    steps: Vec<TimingStep>,
}

#[derive(Serialize)]
struct TimingStep {
    name: String,
    count: u64,
    #[serde(with = "serde_millis")]
    total: Duration,
}

#[derive(Clone, Serialize)]
pub struct SizeMetric {
    name: String,
    value: MetricValue,
}

#[derive(Clone, Serialize)]
pub enum MetricValue {
    Count(u64),
    Bytes(u64),
}

impl Reporter {
    fn new() -> Self {
        Self::default()
    }

    pub fn time<T>(&mut self, step_name: &'static str, run: impl FnOnce() -> T) -> T {
        let started_at = Instant::now();
        let result = tracing::info_span!("step", step_name).in_scope(run);
        self.record_span_time(step_name, started_at.elapsed());
        result
    }

    pub fn record_size(&mut self, name: impl Into<String>, value: MetricValue) {
        self.sizes.push(SizeMetric {
            name: name.into(),
            value,
        });
    }

    pub fn record_timing(&mut self, name: impl Into<String>, elapsed: Duration) {
        let name = name.into();
        self.record_span_time(&name, elapsed);
    }

    fn record_span_time(&mut self, name: &str, elapsed: Duration) {
        let entry = self.spans.entry(name.to_owned()).or_default();
        entry.count += 1;
        entry.total += elapsed;
    }

    fn build_report(&self, command: impl Into<String>) -> RunReport {
        let mut steps: Vec<_> = self
            .spans
            .iter()
            .filter(|(name, _)| name.as_str() != "command")
            .map(|(name, stats)| TimingStep {
                name: name.clone(),
                count: stats.count,
                total: stats.total,
            })
            .collect();
        steps.sort_by(|left, right| right.total.cmp(&left.total));

        RunReport {
            command: command.into(),
            timing: TimingReport {
                total: self
                    .spans
                    .get("command")
                    .map(|stats| stats.total)
                    .unwrap_or_default(),
                steps,
            },
            sizes: self.sizes.clone(),
        }
    }
}

pub fn with_report<T>(command: &str, run: impl FnOnce(&mut Reporter) -> T) -> (T, RunReport) {
    let mut reporter = Reporter::new();
    let subscriber = Registry::default();
    let result = tracing::subscriber::with_default(subscriber, || {
        let started_at = Instant::now();
        let result = tracing::info_span!("command", command).in_scope(|| run(&mut reporter));
        reporter.record_span_time("command", started_at.elapsed());
        result
    });
    let report = reporter.build_report(command);
    (result, report)
}

impl Display for RunReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "Report for {}:", self.command)?;
        write!(f, "{}", self.timing)?;
        write_metric_section(f, "sizes", &self.sizes)?;
        Ok(())
    }
}

impl Display for TimingReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "timing:")?;
        writeln!(f, "  total: {}", self.total.as_secs_f64())?;
        if self.steps.is_empty() {
            writeln!(f, "  steps: none recorded")?;
            return Ok(());
        }

        writeln!(f, "  breakdown:")?;
        for step in &self.steps {
            let avg = if step.count == 0 {
                Duration::default()
            } else {
                Duration::from_secs_f64(step.total.as_secs_f64() / step.count as f64)
            };
            writeln!(
                f,
                "    {}: total={}, count={}, avg={}",
                step.name,
                step.total.as_secs_f64(),
                step.count,
                avg.as_secs_f64(),
            )?;
        }
        Ok(())
    }
}

fn write_metric_section<T>(f: &mut Formatter<'_>, title: &str, metrics: &[T]) -> fmt::Result
where
    T: Display,
{
    writeln!(f, "{title}:")?;
    if metrics.is_empty() {
        writeln!(f, "  none")?;
        return Ok(());
    }

    for metric in metrics {
        writeln!(f, "  {metric}")?;
    }
    Ok(())
}

impl Display for SizeMetric {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} = {}", self.name, self.value)
    }
}

impl Display for MetricValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MetricValue::Count(value) => write!(f, "{value}"),
            MetricValue::Bytes(value) => write!(f, "{} bytes", value),
        }
    }
}
