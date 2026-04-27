use std::fmt::{self, Display, Formatter};
use std::time::{Duration, Instant};

use hashbrown::HashMap;
use serde::Serialize;

#[derive(Default)]
pub struct Reporter {
    spans: HashMap<String, SpanStats>,
    sizes: Vec<SizeMetric>,
}

pub struct Timer {
    name: String,
    tags: Vec<String>,
    started_at: Instant,
}

#[derive(Clone, Default)]
struct SpanStats {
    tags: Vec<String>,
    count: u64,
    total: Duration,
}

#[derive(Serialize)]
pub struct RunReport {
    label: String,
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

    pub fn start_timer(&self, name: String, tags: Vec<String>) -> Timer {
        Timer {
            name,
            tags,
            started_at: Instant::now(),
        }
    }

    pub fn finish_timer(&mut self, timer: Timer) {
        let entry = self.spans.entry(timer.name).or_insert_with(|| SpanStats {
            tags: timer.tags.clone(),
            ..SpanStats::default()
        });
        // We aggregate spans by name, so repeated timers for the same name must
        // carry the same tag set.
        debug_assert_eq!(entry.tags, timer.tags);
        entry.count += 1;
        entry.total += timer.started_at.elapsed();
    }

    pub fn record_size(&mut self, name: String, value: MetricValue) {
        self.sizes.push(SizeMetric { name, value });
    }

    pub fn build_report(&self, label: String) -> RunReport {
        let mut steps: Vec<_> = self
            .spans
            .iter()
            .filter(|(name, _)| name.as_str() != "command")
            .map(|(name, stats)| TimingStep {
                name: name.clone(),
                tags: stats.tags.clone(),
                count: stats.count,
                total: stats.total,
            })
            .collect();
        steps.sort_by(|left, right| right.total.cmp(&left.total));

        RunReport {
            label,
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

impl Display for RunReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "Report for {}:", self.label)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_report_aggregates_timers_by_name_and_preserves_tags() {
        let mut reporter = Reporter::new();

        let command_timer = reporter.start_timer("command".to_string(), Vec::new());
        std::thread::sleep(Duration::from_millis(1));
        reporter.finish_timer(command_timer);

        let step_timer_one = reporter.start_timer("step".to_string(), vec!["tag".to_string()]);
        std::thread::sleep(Duration::from_millis(1));
        reporter.finish_timer(step_timer_one);

        let step_timer_two = reporter.start_timer("step".to_string(), vec!["tag".to_string()]);
        std::thread::sleep(Duration::from_millis(1));
        reporter.finish_timer(step_timer_two);

        let report = reporter.build_report("label".to_string());

        assert!(report.timing.total >= Duration::from_millis(1));
        assert_eq!(report.timing.steps.len(), 1);
        assert_eq!(report.timing.steps[0].name, "step");
        assert_eq!(report.timing.steps[0].tags, vec!["tag".to_string()]);
        assert_eq!(report.timing.steps[0].count, 2);
        assert!(report.timing.steps[0].total >= Duration::from_millis(2));
    }
}
