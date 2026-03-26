use crate::{
    ast::ResolvedVar,
    core::{
        GenericAtom, GenericCoreAction, GenericCoreActions, Query, ResolvedCall, ResolvedCoreRule,
    },
    egglog::util::IndexMap,
    term_encoding::EncodingState,
    CommandMacroRegistry, EGraph, RunReport, TypeInfo,
};

/// Generate a json report for the size of a serialized structu
/// By default, only uses serialize
/// Allow specalization to look into subfields

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SizeReport {
    name: String,
    size: usize,
    fields: Vec<(String, Box<SizeReport>)>,
}

fn up_to_two_decimals(a: usize, b: usize) -> String {
    let a100 = a * 100 / b;
    let high = a100 / 100;
    let low = a100 % 100;
    let low_str = if low < 10 {
        "0".to_string() + &low.to_string()
    } else {
        low.to_string()
    };
    high.to_string() + "." + &low_str
}

fn pretty_print_nbytes(size: usize) -> String {
    if size < 200 {
        size.to_string() + "B"
    } else if size < 200 * 1024 {
        up_to_two_decimals(size, 1024) + "KB"
    } else if size < 200 * 1024 * 1024 {
        up_to_two_decimals(size, 1024 * 1024) + "MB"
    } else {
        up_to_two_decimals(size, 1024 * 1024 * 1024) + "GB"
    }
}

fn truncate_string_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let mut truncated = s.chars().take(max_len).collect::<String>();
        truncated.push_str(&format!("...{:} chars total", s.len()));
        truncated
    } else {
        s.to_string()
    }
}

impl SizeReport {
    pub fn pretty_print(&self, level: usize, max_level: usize) {
        if level > max_level {
            return;
        }
        if level == 0 {
            println!("{} : {}", self.name, pretty_print_nbytes(self.size));
        }
        let mut sorted_fields = self.fields.clone();
        sorted_fields.sort_by(|(_, a), (_, b)| b.size.cmp(&a.size));
        for (name, sr) in sorted_fields.iter().take(10) {
            let percentage = (sr.size as f64 / self.size as f64) * 100.0;
            let indent = level * 2;
            println!(
                "  {:indent$}{} : {} ({:.2}%)",
                "",
                name,
                pretty_print_nbytes(sr.size),
                percentage
            );
            if percentage > 1.0 {
                sr.pretty_print(level + 1, max_level);
            }
        }
        if sorted_fields.len() > 10 {
            println!("  {:level$} ... {:} fields total", "", sorted_fields.len());
        }
    }
}

fn get_sizerp_default<T: serde::Serialize>(obj: &T) -> SizeReport {
    let mut buf = flexbuffers::FlexbufferSerializer::new();
    serde::Serialize::serialize(obj, &mut buf).expect("Failed to serialize in Flexbuffer");
    SizeReport {
        name: std::any::type_name::<T>().to_string(),
        size: buf.view().len(),
        fields: Vec::new(),
    }
}

pub trait GenerateSizeReport: serde::Serialize + Sized {
    fn get_sizerp(&self) -> SizeReport {
        get_sizerp_default(self)
    }
}

impl<T: serde::Serialize> GenerateSizeReport for Option<T> {}

impl<K: serde::Serialize + ToString, V: serde::Serialize + GenerateSizeReport> GenerateSizeReport
    for IndexMap<K, V>
{
    fn get_sizerp(&self) -> SizeReport {
        let mut ret = get_sizerp_default(self);
        for (k, v) in self {
            ret.fields.push((
                truncate_string_with_ellipsis(&k.to_string(), 20),
                Box::new(v.get_sizerp()),
            ));
        }
        ret
    }
}

impl GenerateSizeReport for TypeInfo {}

impl GenerateSizeReport for RunReport {}

impl<K: serde::Serialize, V: serde::Serialize + GenerateSizeReport> GenerateSizeReport
    for egglog_numeric_id::DenseIdMap<K, V> {}

impl GenerateSizeReport for CommandMacroRegistry {}

impl GenerateSizeReport for EncodingState {}

impl GenerateSizeReport for egglog::Function {}

use egglog::ast::Ruleset;
use egglog_ast::span::Span;

impl GenerateSizeReport for Span {}

impl<H: serde::Serialize, L: serde::Serialize> GenerateSizeReport for GenericAtom<H, L> {}

impl<H: serde::Serialize, L: serde::Serialize> GenerateSizeReport for Query<H, L> {
    fn get_sizerp(&self) -> SizeReport {
        self.atoms.get_sizerp()
    }
}

impl<T: serde::Serialize + GenerateSizeReport> GenerateSizeReport for Vec<T> {
    fn get_sizerp(&self) -> SizeReport {
        let mut ret = get_sizerp_default(self);
        for e in self {
            let rep = e.get_sizerp();
            ret.fields.push((rep.name.clone(), Box::new(rep)));
        }
        ret
    }
}

impl<H: serde::Serialize, L: serde::Serialize> GenerateSizeReport for GenericCoreAction<H, L> {}

impl<H: serde::Serialize, L: serde::Serialize> GenerateSizeReport for GenericCoreActions<H, L> {
    fn get_sizerp(&self) -> SizeReport {
        self.0.get_sizerp()
    }
}

impl GenerateSizeReport for ResolvedCall {}

impl GenerateSizeReport for ResolvedVar {}

impl GenerateSizeReport for ResolvedCoreRule {
    fn get_sizerp(&self) -> SizeReport {
        let mut ret = get_sizerp_default(self);
        ret.fields
            .push(("span".to_string(), Box::new(self.span.get_sizerp())));
        ret.fields
            .push(("body".to_string(), Box::new(self.body.get_sizerp())));
        ret.fields
            .push(("head".to_string(), Box::new(self.head.get_sizerp())));
        ret
    }
}

impl<T: serde::Serialize + GenerateSizeReport, S: serde::Serialize + GenerateSizeReport>
    GenerateSizeReport for (T, S)
{
    fn get_sizerp(&self) -> SizeReport {
        let mut ret = get_sizerp_default(self);
        ret.fields
            .push(("0".to_string(), Box::new(self.0.get_sizerp())));
        ret.fields
            .push(("1".to_string(), Box::new(self.1.get_sizerp())));
        ret
    }
}

impl GenerateSizeReport for egglog_bridge::RuleId {}

impl GenerateSizeReport for egglog::ast::Ruleset {
    fn get_sizerp(&self) -> SizeReport {
        match &self {
            Ruleset::Rules(mp) => mp.get_sizerp(),
            Ruleset::Combined(_l) => {
                //TODO if needed
                get_sizerp_default(self)
            }
        }
    }
}

impl GenerateSizeReport for EGraph {
    fn get_sizerp(&self) -> SizeReport {
        let mut ret = get_sizerp_default(&self);
        ret.fields
            .push(("backend".to_string(), Box::new(self.backend.get_sizerp())));
        ret.fields.push((
            "pushed_egraph".to_string(),
            Box::new(self.pushed_egraph.get_sizerp()),
        ));
        ret.fields.push((
            "functions".to_string(),
            Box::new(self.functions.get_sizerp()),
        ));
        ret.fields
            .push(("rulesets".to_string(), Box::new(self.rulesets.get_sizerp())));
        ret.fields.push((
            "type_info".to_string(),
            Box::new(self.type_info.get_sizerp()),
        ));
        ret.fields.push((
            "overall_run_report".to_string(),
            Box::new(self.overall_run_report.get_sizerp()),
        ));
        //ret.fields.push((
        //    "schedulers".to_string(),
        //    Box::new(self.schedulers.get_sizerp()),
        //));
        //ret.fields.push(("commands".to_string(), Box::new(self.commands.get_sizerp())));
        //ret.fields.push(("command_macros".to_string(), Box::new(self.command_macros.get_sizerp())));
        ret.fields.push((
            "proof_state".to_string(),
            Box::new(self.proof_state.get_sizerp()),
        ));
        ret
    }
}

impl GenerateSizeReport for egglog_bridge::EGraph {}