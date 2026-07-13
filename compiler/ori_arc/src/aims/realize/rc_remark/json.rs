//! JSONL serialization for RC remark records and stream headers.

use super::{AimsRcRemark, RcRemarkStreamEnvelope};

impl RcRemarkStreamEnvelope {
    /// Render the header as one JSON object without a trailing newline.
    pub(super) fn to_jsonl(&self) -> String {
        format!(
            "{{\"record\":\"header\",\"schema_version\":{},\"compiler_sha\":\"{}\",\
             \"source_file\":\"{}\",\"burden_path\":{}}}",
            self.schema_version,
            json_escape(&self.compiler_sha),
            json_escape(&self.source_file),
            self.burden_path,
        )
    }
}

impl AimsRcRemark {
    /// Render the remark as one JSON object without a trailing newline.
    pub(super) fn to_jsonl(&self) -> String {
        let function = opt_str_field(self.function.as_deref());
        let debug_loc = match &self.debug_loc {
            Some(loc) => format!(
                "{{\"file\":\"{}\",\"line\":{},\"column\":{}}}",
                json_escape(&loc.file),
                loc.line,
                loc.column
            ),
            None => "null".to_string(),
        };
        let exit_block = opt_num_field(self.exit_block.map(i64::from));
        let burden_net = opt_num_field(self.burden_net);
        let cow_mode = opt_str_field(self.cow_mode.as_deref());
        let args = self
            .args
            .iter()
            .map(|arg| format!("\"{}\"", json_escape(arg)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"kind\":\"{}\",\"pass\":\"{}\",\"name\":\"{}\",\"rc_op\":\"{}\",\
             \"function\":{},\"debug_loc\":{},\"ssa_value\":{},\"exit_block\":{},\
             \"cause\":{{\"proof_failure\":\"{}\",\"lattice_dim\":\"{}\",\"detail\":\"{}\"}},\
             \"burden_net\":{},\"args\":[{}],\"cow_mode\":{}}}",
            self.kind.as_str(),
            json_escape(self.pass),
            json_escape(self.name),
            self.rc_op.as_str(),
            function,
            debug_loc,
            self.ssa_value,
            exit_block,
            json_escape(&self.cause.proof_failure),
            self.cause.lattice_dim.as_str(),
            json_escape(&self.cause.detail),
            burden_net,
            args,
            cow_mode,
        )
    }
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

fn opt_str_field(value: Option<&str>) -> String {
    value.map_or_else(
        || "null".to_string(),
        |value| format!("\"{}\"", json_escape(value)),
    )
}

fn opt_num_field(value: Option<i64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}
