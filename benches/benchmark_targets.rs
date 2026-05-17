// Shared benchmark target metadata.
//
// Targets are advisory in this early renderer phase. The fail threshold is
// recorded separately so benchmark output can show correctness-preserving
// performance budgets without turning normal developer bench runs flaky.

#[derive(Debug, Clone, Copy)]
struct BenchTarget {
    group: &'static str,
    case: &'static str,
    target_ms: u64,
    fail_ms: u64,
}

impl BenchTarget {
    const fn new(group: &'static str, case: &'static str, target_ms: u64, fail_ms: u64) -> Self {
        Self {
            group,
            case,
            target_ms,
            fail_ms,
        }
    }

    fn id(self) -> String {
        format!(
            "{} target={}ms fail={}ms",
            self.case, self.target_ms, self.fail_ms
        )
    }
}

fn report_target(target: BenchTarget) {
    eprintln!(
        "bench-target group={} case={} target_ms={} fail_ms={} mode=soft-warning",
        target.group, target.case, target.target_ms, target.fail_ms
    );
}
