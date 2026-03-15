use std::fmt;
use tracing::Level;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

pub struct SimpleSubscriber;

struct FieldVisitor {
    message: Option<String>,
    fields: Vec<(String, String)>,
}

impl FieldVisitor {
    fn new() -> Self {
        Self {
            message: None,
            fields: Vec::new(),
        }
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            self.fields
                .push((field.name().to_string(), format!("{value:?}")));
        }
    }
}

const MAX_LEVEL: Level = Level::DEBUG;

fn level_color(level: &Level) -> &'static str {
    match level {
        &Level::TRACE => "\x1b[94m",
        &Level::DEBUG => "\x1b[34m",
        &Level::INFO => "\x1b[32m",
        &Level::WARN => "\x1b[33m",
        &Level::ERROR => "\x1b[31m",
    }
}

fn level_injection(level: &Level) -> String {
    let color = level_color(level);
    let level_str = format!("[{level}]");
    format!("{color}{level_str:<7}\x1b[0m")
}

impl Subscriber for SimpleSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        *metadata.level() <= MAX_LEVEL
    }

    fn new_span(&self, _attrs: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let meta = event.metadata();

        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);

        let level = meta.level();
        let target = meta.target();

        let message = visitor.message.unwrap_or_default();

        print!("{} {target}: {message}", level_injection(level));

        for (k, v) in visitor.fields {
            print!(" {k}={v}");
        }

        println!();
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}
