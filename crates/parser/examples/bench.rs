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

use std::time::Instant;

fn main() {
    // Generate a large SMT-LIB script
    let mut script = String::new();
    script.push_str("(set-logic QF_LIA)\n");

    let num_vars = 50_000;
    for i in 0..num_vars {
        script.push_str(&format!("(declare-fun x{} () Int)\n", i));
    }
    for i in 0..num_vars {
        script.push_str(&format!("(assert (> x{} 0))\n", i));
    }
    script.push_str("(check-sat)\n");

    let size_mb = script.len() as f64 / 1_000_000.0;
    println!(
        "Generated {:.1} MB SMT-LIB script ({} vars)",
        size_mb, num_vars
    );

    let start = Instant::now();
    let result = smtlib_parser::parse(&script);
    let parse_time = start.elapsed();

    println!(
        "Parsed in {:.2?} ({} commands, {} diagnostics)",
        parse_time,
        result.script.commands.len(),
        result.diagnostics.len()
    );

    assert!(result.diagnostics.is_empty(), "Should parse without errors");
    println!(
        "Throughput: {:.0} MB/s",
        size_mb / parse_time.as_secs_f64()
    );
}
