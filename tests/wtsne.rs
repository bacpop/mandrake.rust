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
        max_updates: 20,
        repulsion_samples: 1,
        learning_rate: 1.0,
        initial_exaggeration: false,
        threads: 1,
        quiet: true,
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
                max_updates: 1,
                repulsion_samples: 1,
                threads: 1,
                quiet: true,
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
    assert_eq!(initial.completed_updates(), 0);
    assert_eq!(initial.max_updates(), 20);
    assert!(!initial.is_complete());

    let partial = operation.advance(5);
    assert_eq!(partial.completed_updates(), 5);
    assert!(!partial.is_complete());
    assert!(partial.eq().is_finite());

    let complete = operation.advance(usize::MAX);
    assert_eq!(complete.completed_updates(), 20);
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
fn parallel_rounds_complete_within_one_thread_batch_of_target() {
    let mut options = options();
    options.threads = 4;
    options.max_updates = 10;
    let mut operation = EmbeddingOperation::new(graph(), &options).unwrap();
    let progress = operation.advance(1);
    assert_eq!(progress.completed_updates(), 4);
    let progress = operation.advance(usize::MAX);
    assert!(progress.is_complete());
    assert!(progress.completed_updates() >= options.max_updates);
    assert!(progress.completed_updates() < options.max_updates + options.threads);
    assert!(operation.embedding().iter().all(|value| value.is_finite()));
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
    assert_eq!(options.threads, 1);
    assert_eq!(options.max_updates, 1_000_000);
    assert_eq!(
        mandrake::api::EmbeddingInput::new(vec![0], vec![1], vec![0.1], 2, None)
            .unwrap()
            .n_nodes(),
        2
    );
}
