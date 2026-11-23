use crate::case_result::CaseResult;
use crate::error::Error;
use std::path::Path;

pub fn assert_tests_pass(handler_name: &str, path: &Path, results: &[CaseResult]) {
    let (failed, _, skipped_known_failures) = categorize_results(results);

    println!("{} in {}", execution_summary(results), path.display());

    if failed.len() + skipped_known_failures.len() > 0 {
        print_results(handler_name, &failed, &skipped_known_failures, results);
        if !failed.is_empty() {
            panic!("Tests failed (see above)");
        }
    }
}

fn execution_summary(results: &[CaseResult]) -> String {
    let (failed, skipped_bls, skipped_known_failures) = categorize_results(results);
    let passed = results.len() - failed.len() - skipped_bls.len() - skipped_known_failures.len();
    let crypto = if cfg!(feature = "fake_crypto") {
        "fixture_fake"
    } else {
        "real"
    };
    format!(
        "{} total, {} executed, {} passed, {} failed, {} skipped (BLS), {} skipped (known failure), crypto={}",
        results.len(),
        passed + failed.len(),
        passed,
        failed.len(),
        skipped_bls.len(),
        skipped_known_failures.len(),
        crypto
    )
}

pub fn categorize_results(
    results: &[CaseResult],
) -> (Vec<&CaseResult>, Vec<&CaseResult>, Vec<&CaseResult>) {
    let mut failed = vec![];
    let mut skipped_bls = vec![];
    let mut skipped_known_failures = vec![];

    for case in results {
        match case.result.as_ref().err() {
            Some(Error::SkippedBls) => skipped_bls.push(case),
            Some(Error::SkippedKnownFailure) => skipped_known_failures.push(case),
            Some(_) => failed.push(case),
            None => (),
        }
    }

    (failed, skipped_bls, skipped_known_failures)
}

pub fn print_results(
    handler_name: &str,
    failed: &[&CaseResult],
    skipped_known_failures: &[&CaseResult],
    results: &[CaseResult],
) {
    println!("--------------------------------------------------");
    println!(
        "Test {}",
        if failed.is_empty() {
            "Result"
        } else {
            "Failure"
        }
    );
    println!("Title: {}", handler_name);
    println!("{}", execution_summary(results));
    println!();

    for case in skipped_known_failures {
        println!("-------");
        println!(
            "case ({}) from {} skipped because it's a known failure",
            case.desc,
            case.path.display()
        );
    }
    for failure in failed {
        let error = failure.result.clone().unwrap_err();

        println!("-------");
        println!(
            "case {} ({}) from {} failed with {}:",
            failure.case_index,
            failure.desc,
            failure.path.display(),
            error.name()
        );
        println!("{}", error.message());
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn case(result: Result<(), Error>) -> CaseResult {
        CaseResult {
            case_index: 0,
            desc: "receipt control".into(),
            path: PathBuf::from("fixture"),
            result,
        }
    }

    #[test]
    fn mixed_results_never_count_skips_as_passed() {
        let cases = [
            case(Ok(())),
            case(Err(Error::SkippedBls)),
            case(Err(Error::SkippedKnownFailure)),
            case(Err(Error::NotEqual("actual failure".into()))),
        ];
        let output = execution_summary(&cases);
        assert!(output.starts_with("4 total, 2 executed, 1 passed, 1 failed, 1 skipped (BLS), 1 skipped (known failure)"), "{output}");
        assert!(output.contains(if cfg!(feature = "fake_crypto") {
            "crypto=fixture_fake"
        } else {
            "crypto=real"
        }));
    }

    #[test]
    fn all_skipped_and_empty_have_no_executed_passes() {
        let skipped = [
            case(Err(Error::SkippedBls)),
            case(Err(Error::SkippedKnownFailure)),
        ];
        assert!(execution_summary(&skipped).starts_with("2 total, 0 executed, 0 passed, 0 failed"));
        assert!(execution_summary(&[]).starts_with("0 total, 0 executed, 0 passed, 0 failed"));
    }

    #[test]
    #[should_panic(expected = "Tests failed")]
    fn actual_validation_failure_remains_fatal() {
        assert_tests_pass(
            "failure control",
            Path::new("fixture"),
            &[case(Err(Error::NotEqual("actual failure".into())))],
        );
    }

    #[test]
    fn mode_and_known_skips_remain_distinct_from_validation_failures() {
        let cases = [
            case(Err(Error::SkippedBls)),
            case(Err(Error::SkippedKnownFailure)),
        ];
        let (failed, bls, known) = categorize_results(&cases);
        assert!(failed.is_empty());
        assert_eq!(bls.len(), 1);
        assert_eq!(known.len(), 1);
        assert_tests_pass("skip control", Path::new("fixture"), &cases);
    }
}
