use mandrake::{EmbeddingInput, EmbeddingOperation, WtsneOptions, wtsne};

fn graph() -> EmbeddingInput {
    EmbeddingInput::new(
        vec![0, 0, 1, 1, 2, 2, 3, 3],
        vec![1, 2, 0, 2, 0, 3, 0, 2],
        vec![0.1, 0.4, 0.1, 0.2, 0.4, 0.3, 0.3, 0.2],
        4,
        Some(vec![1.0, 2.0, 1.0, 1.0]),
    )
    .unwrap()
}

fn options() -> WtsneOptions {
    WtsneOptions {
        perplexity: 2.0,
        max_iterations: 20,
        repulsion_samples: 1,
        learning_rate: 1.0,
        initial_exaggeration: false,
        workers: 1,
        seed: 42,
    }
}

#[test]
fn input_moves_owned_data_and_creates_uniform_weights() {
    let input = EmbeddingInput::new(vec![0, 1], vec![1, 0], vec![0.1, 0.1], 2, None).unwrap();
    assert_eq!(input.n_nodes(), 2);
    assert!(
        EmbeddingOperation::new(
            input,
            &WtsneOptions {
                max_iterations: 1,
                repulsion_samples: 1,
                workers: 1,
                ..WtsneOptions::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn operation_exposes_initial_state_and_lifecycle_progress() {
    let mut operation = EmbeddingOperation::new(graph(), &options()).unwrap();
    assert_eq!(operation.embedding().shape(), &[4, 2]);
    assert!(operation.embedding().iter().all(|value| value.is_finite()));

    let initial = operation.advance(0);
    assert_eq!(initial.completed_iterations(), 0);
    assert_eq!(initial.max_iterations(), 20);
    assert!(!initial.is_complete());

    let partial = operation.advance(5);
    assert_eq!(partial.completed_iterations(), 5);
    assert!(!partial.is_complete());
    assert!(partial.eq().is_finite());

    let complete = operation.advance(usize::MAX);
    assert_eq!(complete.completed_iterations(), 20);
    assert!(complete.is_complete());
    assert!(operation.embedding().iter().all(|value| value.is_finite()));

    let after_complete = operation.advance(1);
    assert_eq!(after_complete, complete);
}

#[test]
fn blocking_wrapper_returns_finite_two_dimensional_embedding() {
    let embedding = wtsne(graph(), &options()).unwrap();
    assert_eq!(embedding.shape(), &[4, 2]);
    assert!(embedding.iter().all(|value| value.is_finite()));
}

#[test]
fn fixed_seed_is_reproducible_with_one_worker() {
    let first = wtsne(graph(), &options()).unwrap();
    let second = wtsne(graph(), &options()).unwrap();
    assert_eq!(first, second);
}

#[test]
fn budget_partitioning_preserves_one_worker_result() {
    let mut single_budget = EmbeddingOperation::new(graph(), &options()).unwrap();
    single_budget.advance(20);
    let single = single_budget.into_embedding();

    let mut partitioned = EmbeddingOperation::new(graph(), &options()).unwrap();
    partitioned.advance(3);
    partitioned.advance(7);
    partitioned.advance(10);
    let partitioned = partitioned.into_embedding();

    assert_eq!(single, partitioned);
}

#[test]
fn changing_seed_changes_reproducible_embedding() {
    let mut baseline = options();
    baseline.max_iterations = 1;
    let mut changed = baseline.clone();
    changed.seed += 1;
    assert_ne!(
        wtsne(graph(), &baseline).unwrap(),
        wtsne(graph(), &changed).unwrap()
    );
}

#[test]
fn raw_similarity_mode_is_supported() {
    let input = EmbeddingInput::new(
        vec![0, 0, 1, 1, 2, 2, 3, 3],
        vec![1, 2, 0, 2, 0, 3, 0, 2],
        vec![0.9, 0.6, 0.9, 0.8, 0.6, 0.7, 0.7, 0.8],
        4,
        Some(vec![1.0, 2.0, 1.0, 1.0]),
    )
    .unwrap();
    let mut raw_options = options();
    raw_options.perplexity = 0.0;
    assert!(
        wtsne(input, &raw_options)
            .unwrap()
            .iter()
            .all(|value| value.is_finite())
    );
}

#[test]
fn input_rejects_only_structural_mismatches() {
    let error = EmbeddingInput::new(vec![0], vec![], vec![0.1], 2, None).unwrap_err();
    assert!(error.to_string().contains("same length"));

    let error = EmbeddingInput::new(vec![0], vec![1], vec![0.1], 2, Some(vec![1.0])).unwrap_err();
    assert!(error.to_string().contains("declared node count"));
}

#[test]
fn incomplete_operation_can_transfer_its_partial_embedding() {
    let mut operation = EmbeddingOperation::new(graph(), &options()).unwrap();
    operation.advance(1);
    let embedding = operation.into_embedding();
    assert_eq!(embedding.shape(), &[4, 2]);
}

#[test]
fn public_api_module_exposes_operation_types() {
    let options = mandrake::api::WtsneOptions::default();
    assert_eq!(options.seed, 1);
    assert_eq!(
        mandrake::api::EmbeddingInput::new(vec![0], vec![1], vec![0.1], 2, None)
            .unwrap()
            .n_nodes(),
        2
    );
}
