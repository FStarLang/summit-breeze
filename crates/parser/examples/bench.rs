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
