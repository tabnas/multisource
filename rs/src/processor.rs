/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

//! Turning resolved source text into a value.
//!
//! The default table is the TypeScript one: raw text for an unknown
//! kind, a strict JSON read for `json`, and a re-parse with the live
//! engine for `jsonic` and `jsc`.

use std::fmt;
use std::sync::Arc;

use tabnas::{Tabnas, TabnasError, Value};

use crate::{MultiSourceOptions, Resolution};

/// How many levels of nesting share one stack.
///
/// A nested source is parsed inside the parse that referenced it, and a
/// parse frame is large: a debug build of the engine uses around a third
/// of a megabyte per level, so six levels exhaust the two megabytes a
/// spawned thread gets by default and the process aborts. Neither the
/// canonical TypeScript nor the Go port has this problem in the same
/// way (a JavaScript engine throws a catchable RangeError; a goroutine
/// stack grows), so the port solves it rather than inheriting it: every
/// `SEGMENT` levels the nested parse moves to a fresh thread with a
/// generous stack, and the chain is then bounded by
/// [`crate::MultiSourceOptions::max_depth`] rather than by whatever
/// stack the caller happened to have.
const SEGMENT: usize = 2;

/// The stack a segment thread gets. Virtual address space, committed
/// only as it is used.
const SEGMENT_STACK: usize = 8 * 1024 * 1024;

/// What a [`Processor`] is given.
///
/// The canonical TypeScript hands a processor the whole parse
/// `Context`, of which processors read only `ctx.meta`, plus the
/// instance. Narrowing it keeps a processor callable, and therefore
/// testable, outside a parse.
#[derive(Clone, Copy)]
pub struct ProcessorInput<'a> {
    /// The plugin options in force for this parse.
    pub options: &'a MultiSourceOptions,
    /// The metadata for this load. `multisource.path` names the source
    /// being processed, which is how a relative reference inside it
    /// resolves against its own directory.
    pub meta: &'a Value,
    /// The parser the document is being read by, for a processor that
    /// re-parses its source.
    pub parser: &'a Arc<Tabnas>,
    /// How many sources enclose this one. Zero for a reference in the
    /// document the caller handed to `parse`.
    pub depth: usize,
}

impl ProcessorInput<'_> {
    /// Re-parse `src` as a nested source of this load.
    ///
    /// Use this rather than calling the parser directly: it threads the
    /// metadata through, so a relative reference inside `src` resolves
    /// against this source's own directory, and it keeps the chain off
    /// one stack. See [`parse_nested`].
    pub fn parse_source(&self, src: &str) -> Result<Value, TabnasError> {
        parse_nested(self.parser, src, self.meta, self.depth)
    }
}

/// Parse `src` as a source nested `depth` levels inside the document.
///
/// Every [`SEGMENT`] levels the parse is run on a fresh thread with a
/// large stack and joined, so a long chain of sources cannot exhaust
/// the caller's stack. The call is synchronous either way: nothing
/// outlives it, and the value comes back on the caller's thread.
pub fn parse_nested(
    parser: &Arc<Tabnas>,
    src: &str,
    meta: &Value,
    depth: usize,
) -> Result<Value, TabnasError> {
    if depth % SEGMENT != 1 {
        return parser.parse_with_meta(src, meta.clone());
    }

    let handle = match spawn_segment(parser, src, meta) {
        Ok(handle) => handle,
        Err(reason) => {
            // No thread to be had. Carrying on where we are is not the
            // safe fallback it looks like: the measurements this module
            // records put about six levels in a spawned thread's stack,
            // well inside the default `max_depth`, so resuming the
            // chain here is how an ACYCLIC document aborts the process,
            // and it does it exactly when the machine is already under
            // pressure. Fail the parse instead, with a reason.
            let mut error = TabnasError::new("internal", src, src, 0, 1, 1);
            error.detail =
                format!("the nested parse could not be given a thread to run on: {reason}");
            return Err(error);
        }
    };

    match handle.join() {
        Ok(result) => result,
        Err(_) => {
            // The engine converts a panic inside a parse into an error
            // itself, so reaching here means something outside it, and
            // the parse has no result to report.
            let mut error = TabnasError::new("internal", src, src, 0, 1, 1);
            error.detail = "the nested parse did not complete".to_string();
            Err(error)
        }
    }
}

/// Run one segment of the chain on a thread of its own.
///
/// A seam rather than a call, so the test below can exercise the failure
/// path without having to exhaust the process's threads to reach it.
fn spawn_segment(
    parser: &Arc<Tabnas>,
    src: &str,
    meta: &Value,
) -> std::io::Result<std::thread::JoinHandle<Result<Value, TabnasError>>> {
    #[cfg(test)]
    {
        if REFUSE_SPAWN.with(std::cell::Cell::get) {
            return Err(std::io::Error::other("refused by a test"));
        }
    }

    let owned_parser = Arc::clone(parser);
    let owned_src = src.to_string();
    let owned_meta = meta.clone();
    std::thread::Builder::new()
        .name("tabnas-multisource".to_string())
        .stack_size(SEGMENT_STACK)
        .spawn(move || owned_parser.parse_with_meta(&owned_src, owned_meta))
}

#[cfg(test)]
thread_local! {
    /// Set by the test that pins the spawn-failure path.
    static REFUSE_SPAWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl fmt::Debug for ProcessorInput<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessorInput")
            .field("meta", self.meta)
            .finish_non_exhaustive()
    }
}

/// Convert resolved source content into a value.
///
/// Write the result to `resolution.val`. A failure goes in
/// `resolution.err`, which fails the whole parse: TypeScript lets a
/// nested parse error escape rather than substituting the raw text, and
/// so does this.
///
/// Implementations must be `Send + Sync`, for the reason [`crate::Resolver`]
/// gives.
pub trait Processor: Send + Sync {
    /// Process `resolution` in place.
    fn process(&self, resolution: &mut Resolution, input: &ProcessorInput<'_>);
}

impl<F> Processor for F
where
    F: Fn(&mut Resolution, &ProcessorInput<'_>) + Send + Sync,
{
    fn process(&self, resolution: &mut Resolution, input: &ProcessorInput<'_>) {
        self(resolution, input);
    }
}

/// The fallback: the source text, as a string.
pub fn default_processor(resolution: &mut Resolution, _input: &ProcessorInput<'_>) {
    resolution.val = match &resolution.src {
        Some(src) => Value::String(src.clone()),
        None => Value::Undefined,
    };
}

/// Re-parse the source with the live engine, so a nested reference is
/// resolved by the same plugin and every plugin installed beside it.
///
/// The metadata is threaded through, which is what makes a relative
/// reference inside the source resolve against that source's own
/// directory. Mirrors the TypeScript `jsonic` processor's
/// `tn.parse(res.src, ctx.meta)`.
pub fn jsonic_processor(resolution: &mut Resolution, input: &ProcessorInput<'_>) {
    let (Some(src), Some(_)) = (resolution.src.clone(), resolution.spec.full.clone()) else {
        // The canonical processor leaves `val` untouched when either is
        // missing, and the rule's node is then the empty map its `bo`
        // hook seeded.
        return;
    };
    match input.parse_source(&src) {
        Ok(value) => resolution.val = value,
        Err(error) => {
            // Keep the raw text for a caller inspecting the resolution
            // directly; the error is what fails the parse.
            resolution.val = Value::String(src);
            resolution.err = Some(Box::new(error));
        }
    }
}

/// Read the source as standard JSON.
///
/// Malformed JSON fails the parse, matching the canonical TypeScript
/// `json` processor, which reads through a strict-JSON jsonic instance
/// and lets its error escape. Substituting the raw text instead hid the
/// broken file and handed the caller a string where a map was expected.
pub fn json_processor(resolution: &mut Resolution, _input: &ProcessorInput<'_>) {
    let Some(src) = resolution.src.clone() else {
        resolution.val = Value::Undefined;
        return;
    };
    // A fresh strict instance per call would rebuild the JSON grammar
    // every time; the shared one is immutable and parses through `&self`.
    match strict_json().parse(&src) {
        Ok(value) => resolution.val = value,
        Err(error) => {
            resolution.val = Value::String(src);
            resolution.err = Some(Box::new(error));
        }
    }
}

/// The shared strict-JSON reader, the Rust counterpart of the module
/// level `Jsonic.make('json')` the TypeScript processor closes over.
fn strict_json() -> &'static Tabnas {
    use std::sync::OnceLock;
    static PARSER: OnceLock<Tabnas> = OnceLock::new();
    PARSER.get_or_init(tabnas_jsonic::make_json)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A process at its thread limit must not resume the nested chain on
    /// the current stack. Six levels fit a spawned thread's two
    /// megabytes, and `max_depth` allows sixty-four, so the fallback
    /// turned "no thread available" into an aborted process for a
    /// perfectly acyclic document. It is a parse error instead.
    #[test]
    fn a_segment_that_cannot_be_spawned_fails_the_parse() {
        let parser = Arc::new(crate::make());
        let meta = Value::Undefined;

        REFUSE_SPAWN.with(|refuse| refuse.set(true));
        let error = parse_nested(&parser, "a:1", &meta, 1)
            .expect_err("a segment with no thread to run on is an error");
        assert!(!error.code.is_empty(), "the failure carries a code");
        assert!(
            error.detail.contains("thread"),
            "the failure says why: {}",
            error.detail
        );

        // A level that shares the caller's stack is unaffected: it never
        // asks for a thread.
        assert!(parse_nested(&parser, "a:1", &meta, 0).is_ok());

        REFUSE_SPAWN.with(|refuse| refuse.set(false));
        assert!(
            parse_nested(&parser, "a:1", &meta, 1).is_ok(),
            "and with a thread to be had the segment runs"
        );
    }
}
