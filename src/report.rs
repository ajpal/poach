use std::fmt::{self, Display, Formatter};
use std::time::{Duration, Instant};

use hashbrown::HashMap;
use serde::Serialize;

#[derive(Default)]
pub struct Reporter {
    spans: HashMap<(String, Vec<String>), SpanStats>,
    sizes: Vec<SizeMetric>,
}

pub struct Timer {
    name: String,
    started_at: Instant,
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
    tags: Vec<String>,
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_timer(&self, name: String) -> Timer {
        Timer {
            name,
            started_at: Instant::now(),
        }
    }

    pub fn finish_timer(&mut self, timer: Timer) {
        self.record_span_time(&timer.name, &[], timer.started_at.elapsed());
    }

    pub fn record_size(&mut self, name: String, value: MetricValue) {
        self.sizes.push(SizeMetric { name, value });
    }

    pub fn record_timing(&mut self, name: String, tags: Vec<String>, elapsed: Duration) {
        self.record_span_time(&name, &tags, elapsed);
    }

    fn record_span_time(&mut self, name: &str, tags: &[String], elapsed: Duration) {
        let entry = self
            .spans
            .entry((name.to_owned(), tags.to_vec()))
            .or_default();
        entry.count += 1;
        entry.total += elapsed;
    }

    pub fn build_report(&self, command: String) -> RunReport {
        let mut steps: Vec<_> = self
            .spans
            .iter()
            .filter(|((name, _), _)| name.as_str() != "command")
            .map(|((name, tags), stats)| TimingStep {
                name: name.clone(),
                tags: tags.clone(),
                count: stats.count,
                total: stats.total,
            })
            .collect();
        steps.sort_by(|left, right| right.total.cmp(&left.total));

        RunReport {
            command,
            timing: TimingReport {
                total: self
                    .spans
                    .get(&("command".to_string(), vec![]))
                    .map(|stats| stats.total)
                    .unwrap_or_default(),
                steps,
            },
            sizes: self.sizes.clone(),
        }
    }
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
            let tag_display = if step.tags.is_empty() {
                "untagged".to_string()
            } else {
                step.tags.join(",")
            };
            writeln!(
                f,
                "    {} [{}]: total={}, count={}, avg={}",
                step.name,
                tag_display,
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
