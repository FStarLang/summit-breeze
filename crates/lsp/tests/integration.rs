// Copyright 2026 Microsoft Research
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use smtlib_parser::parse;
use summit_breeze_lsp::symbols::{build_index, CommandInfoKind, SymbolKind};

/// A realistic SMT-LIB script used across multiple tests.
const REALISTIC_SCRIPT: &str = r#"
(set-logic QF_LIA)
(declare-fun x () Int)
(declare-fun y () Int)
(define-fun add ((a Int) (b Int)) Int (+ a b))
(assert (> (add x y) 0))
(push 1)
(declare-fun z () Int)
(assert (< z 10))
(check-sat)
(pop 1)
(check-sat)
"#;

// -----------------------------------------------------------------------
// 1. Parse a realistic script and build the index
// -----------------------------------------------------------------------

#[test]
fn parse_realistic_script() {
    let result = parse(REALISTIC_SCRIPT);
    assert!(
        result.diagnostics.is_empty(),
        "Expected no diagnostics, got: {:?}",
        result.diagnostics
    );
    let index = build_index(&result.script);
    // Should have definitions for x, y, add, z, plus params a and b
    assert!(index.definitions.contains_key("x"));
    assert!(index.definitions.contains_key("y"));
    assert!(index.definitions.contains_key("add"));
    assert!(index.definitions.contains_key("z"));
    assert!(index.definitions.contains_key("a"));
    assert!(index.definitions.contains_key("b"));
}

// -----------------------------------------------------------------------
// 2. Go-to-definition: find a reference, verify definition's name_span
// -----------------------------------------------------------------------

#[test]
fn goto_definition() {
    let result = parse(REALISTIC_SCRIPT);
    let index = build_index(&result.script);

    // Find a reference to "x" in the assert (add x y)
    let x_refs: Vec<_> = index.references.iter().filter(|r| r.name == "x").collect();
    assert!(!x_refs.is_empty(), "Should have references to x");

    // The definition of x should exist with a valid name_span
    let x_defs = &index.definitions["x"];
    assert_eq!(x_defs.len(), 1);
    let x_def = &x_defs[0];
    assert_eq!(x_def.kind, SymbolKind::Function);

    // The name_span should point to "x" in the source
    let name_text = &REALISTIC_SCRIPT[x_def.name_span.start as usize..x_def.name_span.end as usize];
    assert_eq!(name_text, "x");
}

// -----------------------------------------------------------------------
// 3. Find-references: verify all reference spans for a declared function
// -----------------------------------------------------------------------

#[test]
fn find_references() {
    let result = parse(REALISTIC_SCRIPT);
    let index = build_index(&result.script);

    // "add" is declared via define-fun and referenced in the assert
    let add_refs: Vec<_> = index.references.iter().filter(|r| r.name == "add").collect();
    assert!(
        !add_refs.is_empty(),
        "Should have at least one reference to add"
    );

    // Verify each reference span starts at the right position in the source
    for r in &add_refs {
        let start_text = &REALISTIC_SCRIPT[r.span.start as usize..];
        assert!(
            start_text.starts_with("add"),
            "Reference span should start with 'add', got: {:?}",
            &start_text[..20.min(start_text.len())]
        );
    }

    // "x" should be referenced (used in the assert)
    let x_refs: Vec<_> = index.references.iter().filter(|r| r.name == "x").collect();
    assert!(!x_refs.is_empty());
    for r in &x_refs {
        let start_text = &REALISTIC_SCRIPT[r.span.start as usize..];
        assert!(
            start_text.starts_with("x"),
            "Reference span should start with 'x'"
        );
    }
}

// -----------------------------------------------------------------------
// 4. Push/pop pairs are correctly matched
// -----------------------------------------------------------------------

#[test]
fn push_pop_pairs() {
    let result = parse(REALISTIC_SCRIPT);
    let index = build_index(&result.script);

    assert_eq!(index.push_pop_pairs.len(), 1, "Should have one push/pop pair");
    let pair = &index.push_pop_pairs[0];
    assert!(pair.pop_span.is_some(), "Pop span should be present");
    assert_eq!(pair.depth, 0, "Push at depth 0");

    // Verify the spans point to the right text
    let push_text =
        &REALISTIC_SCRIPT[pair.push_span.start as usize..pair.push_span.end as usize];
    assert!(push_text.contains("push"), "Push span should contain 'push'");

    let pop_span = pair.pop_span.unwrap();
    let pop_text = &REALISTIC_SCRIPT[pop_span.start as usize..pop_span.end as usize];
    assert!(pop_text.contains("pop"), "Pop span should contain 'pop'");
}

// -----------------------------------------------------------------------
// 5. Folding: command_spans cover all expected commands
// -----------------------------------------------------------------------

#[test]
fn folding_command_spans() {
    let result = parse(REALISTIC_SCRIPT);
    let index = build_index(&result.script);

    // Count expected commands: set-logic, 2x declare-fun, define-fun, assert,
    // push, declare-fun, assert, check-sat, pop, check-sat = 11
    assert!(
        index.command_spans.len() >= 10,
        "Expected at least 10 command spans, got {}",
        index.command_spans.len()
    );

    // Each command span should be non-empty
    for cmd in &index.command_spans {
        assert!(
            cmd.span.start < cmd.span.end,
            "Command span should be non-empty: {:?}",
            cmd
        );
    }
}

// -----------------------------------------------------------------------
// 6. Outline: CommandInfo entries have correct kinds
// -----------------------------------------------------------------------

#[test]
fn outline_command_kinds() {
    let result = parse(REALISTIC_SCRIPT);
    let index = build_index(&result.script);

    let kinds: Vec<CommandInfoKind> = index.command_spans.iter().map(|c| c.kind).collect();

    assert!(kinds.contains(&CommandInfoKind::SetLogic));
    assert!(kinds.contains(&CommandInfoKind::DeclareFun));
    assert!(kinds.contains(&CommandInfoKind::DefineFun));
    assert!(kinds.contains(&CommandInfoKind::Assert));
    assert!(kinds.contains(&CommandInfoKind::Push));
    assert!(kinds.contains(&CommandInfoKind::Pop));
    assert!(kinds.contains(&CommandInfoKind::CheckSat));

    // Verify named commands have the right names
    let set_logic = index
        .command_spans
        .iter()
        .find(|c| c.kind == CommandInfoKind::SetLogic)
        .unwrap();
    assert_eq!(set_logic.name.as_deref(), Some("QF_LIA"));

    let define_fun = index
        .command_spans
        .iter()
        .find(|c| c.kind == CommandInfoKind::DefineFun)
        .unwrap();
    assert_eq!(define_fun.name.as_deref(), Some("add"));
}

// -----------------------------------------------------------------------
// 7. Error recovery: parse a script with intentional errors
// -----------------------------------------------------------------------

#[test]
fn error_recovery() {
    let broken_script = r#"
(set-logic QF_LIA)
(declare-fun x () Int)
(assert (> x
(declare-fun y () Int)
(assert (= y 42))
(check-sat)
"#;

    let result = parse(broken_script);

    // Should have diagnostics for the broken assert
    assert!(
        !result.diagnostics.is_empty(),
        "Should report parse errors"
    );

    // Despite errors, some commands should still parse (error recovery)
    assert!(
        !result.script.commands.is_empty(),
        "Should recover some commands even with errors"
    );

    // Build index on the partially parsed script — should not panic
    let index = build_index(&result.script);

    // We should at least find the declarations that parsed successfully
    let has_some_defs = !index.definitions.is_empty();
    assert!(has_some_defs, "Should have some definitions from recovered commands");
}

// -----------------------------------------------------------------------
// 8. Nested let/forall/exists in a larger script
// -----------------------------------------------------------------------

#[test]
fn nested_let_forall_exists() {
    let script = r#"
(set-logic UFLIA)
(declare-sort U 0)
(declare-fun f (U) Int)
(declare-fun g (Int Int) Int)
(declare-fun p (Int) Bool)

(define-fun complex ((x U)) Bool
  (let ((fx (f x)))
    (forall ((y Int))
      (exists ((z Int))
        (and
          (p (g fx z))
          (> (g y z) 0))))))

(assert (forall ((u U)) (complex u)))
(check-sat)
"#;

    let result = parse(script);
    assert!(
        result.diagnostics.is_empty(),
        "Expected no diagnostics, got: {:?}",
        result.diagnostics
    );

    let index = build_index(&result.script);

    // Definitions: U (sort), f, g, p (functions), complex (function), x (variable param)
    assert!(index.definitions.contains_key("U"));
    assert!(index.definitions.contains_key("f"));
    assert!(index.definitions.contains_key("g"));
    assert!(index.definitions.contains_key("p"));
    assert!(index.definitions.contains_key("complex"));
    assert!(index.definitions.contains_key("x"));

    // References should include uses inside nested let/forall/exists
    let f_refs: Vec<_> = index.references.iter().filter(|r| r.name == "f").collect();
    assert!(!f_refs.is_empty(), "Should reference f inside let binding");

    let g_refs: Vec<_> = index.references.iter().filter(|r| r.name == "g").collect();
    assert!(
        g_refs.len() >= 2,
        "Should reference g at least twice, got {}",
        g_refs.len()
    );

    // Quantifier-bound variables should appear as references
    let y_refs: Vec<_> = index.references.iter().filter(|r| r.name == "y").collect();
    assert!(!y_refs.is_empty(), "forall-bound y should be referenced");

    let z_refs: Vec<_> = index.references.iter().filter(|r| r.name == "z").collect();
    assert!(!z_refs.is_empty(), "exists-bound z should be referenced");

    // Let-bound variable fx
    let fx_refs: Vec<_> = index.references.iter().filter(|r| r.name == "fx").collect();
    assert!(!fx_refs.is_empty(), "let-bound fx should be referenced");

    // The assert references "complex" and the forall-bound "u"
    let complex_refs: Vec<_> = index
        .references
        .iter()
        .filter(|r| r.name == "complex")
        .collect();
    assert!(
        !complex_refs.is_empty(),
        "Should reference complex in assert"
    );

    let u_refs: Vec<_> = index.references.iter().filter(|r| r.name == "u").collect();
    assert!(!u_refs.is_empty(), "forall-bound u should be referenced");
}

// -----------------------------------------------------------------------
// 9. Stack depth tracking across push/pop
// -----------------------------------------------------------------------

#[test]
fn stack_depth_tracking() {
    let script = r#"
(declare-fun a () Int)
(push 1)
(declare-fun b () Int)
(push 1)
(declare-fun c () Int)
(pop 1)
(pop 1)
"#;

    let result = parse(script);
    assert!(result.diagnostics.is_empty());
    let index = build_index(&result.script);

    // a is at depth 0, b at depth 1, c at depth 2
    assert_eq!(index.definitions["a"][0].stack_depth, 0);
    assert_eq!(index.definitions["b"][0].stack_depth, 1);
    assert_eq!(index.definitions["c"][0].stack_depth, 2);

    // Two push/pop pairs
    assert_eq!(index.push_pop_pairs.len(), 2);
}
