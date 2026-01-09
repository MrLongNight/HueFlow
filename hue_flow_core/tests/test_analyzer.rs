use hue_flow_core::audio::analyzer::FftAnalyzer;

#[test]
fn test_analyzer_creation_and_processing() {
    let mut analyzer = FftAnalyzer::new(1024);
    // 1024 samples of silence
    let samples = vec![0.0; 1024];
    let spectrum = analyzer.process(&samples);

    assert_eq!(spectrum.bass, 0.0);
    assert_eq!(spectrum.mids, 0.0);
    assert_eq!(spectrum.highs, 0.0);
    assert_eq!(spectrum.energy, 0.0);
}

#[test]
fn test_analyzer_with_sine_wave() {
    let mut analyzer = FftAnalyzer::new(1024);
    analyzer.set_sampling_rate(44100);

    // Generate 100Hz Sine Wave (Bass)
    let mut samples = Vec::new();
    for i in 0..1024 {
        let t = i as f32 / 44100.0;
        samples.push((2.0 * std::f32::consts::PI * 100.0 * t).sin());
    }

    // Process multiple times to let AGC stabilize?
    // AGC starts at 0.01. Input amp is 1.0. FFT amp will be significant.
    // Normalized bass should be near 1.0.

    let spectrum = analyzer.process(&samples);
    println!("Bass: {}, Mids: {}, Highs: {}, Energy: {}", spectrum.bass, spectrum.mids, spectrum.highs, spectrum.energy);

    assert!(spectrum.bass > 0.5);
    assert!(spectrum.mids < 0.2); // Leakage might occur but should be low
    assert!(spectrum.highs < 0.1);
}
