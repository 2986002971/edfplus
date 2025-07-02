// Internal utilities for documentation tests
// This file contains helper functions to generate test files for doctests

use crate::{EdfWriter, SignalParam, Result};
use std::path::Path;

/// Creates a simple test EDF+ file for documentation examples
pub fn create_simple_test_file<P: AsRef<Path>>(path: P) -> Result<()> {
    let mut writer = EdfWriter::create(&path)?;
    
    // Set patient info
    writer.set_patient_info("DOC001", "M", "01-JAN-1990", "Test Patient")?;
    
    // Add a simple EEG signal
    writer.add_signal(SignalParam {
        label: "EEG Fp1".to_string(),
        samples_in_file: 0,
        physical_max: 200.0,
        physical_min: -200.0,
        digital_max: 32767,
        digital_min: -32768,
        samples_per_record: 256,
        physical_dimension: "uV".to_string(),
        prefilter: "HP:0.1Hz LP:70Hz".to_string(),
        transducer: "AgAgCl cup electrodes".to_string(),
    })?;
    
    // Generate one second of 10Hz sine wave data
    let mut samples = Vec::new();
    for i in 0..256 {
        let t = i as f64 / 256.0;
        let value = 50.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin();
        samples.push(value);
    }
    
    writer.write_samples(&[samples])?;
    writer.finalize()?;
    Ok(())
}

/// Creates a multi-channel test EDF+ file for documentation examples
pub fn create_multi_channel_test_file<P: AsRef<Path>>(path: P) -> Result<()> {
    let mut writer = EdfWriter::create(&path)?;
    
    writer.set_patient_info("DOC002", "F", "15-MAR-1985", "Multi Channel Test")?;
    
    // Add EEG signal
    writer.add_signal(SignalParam {
        label: "EEG C3".to_string(),
        samples_in_file: 0,
        physical_max: 200.0,
        physical_min: -200.0,
        digital_max: 32767,
        digital_min: -32768,
        samples_per_record: 256,
        physical_dimension: "uV".to_string(),
        prefilter: "HP:0.1Hz LP:70Hz".to_string(),
        transducer: "AgAgCl electrodes".to_string(),
    })?;
    
    // Add ECG signal
    writer.add_signal(SignalParam {
        label: "ECG Lead II".to_string(),
        samples_in_file: 0,
        physical_max: 5.0,
        physical_min: -5.0,
        digital_max: 32767,
        digital_min: -32768,
        samples_per_record: 256,
        physical_dimension: "mV".to_string(),
        prefilter: "HP:0.1Hz LP:100Hz".to_string(),
        transducer: "Chest electrodes".to_string(),
    })?;
    
    // Generate sample data
    let mut eeg_samples = Vec::new();
    let mut ecg_samples = Vec::new();
    
    for i in 0..256 {
        let t = i as f64 / 256.0;
        
        // EEG: Alpha wave (10 Hz) with noise
        let eeg = 30.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin()
                + 5.0 * (2.0 * std::f64::consts::PI * 50.0 * t).sin();
        eeg_samples.push(eeg);
        
        // ECG: Heart beat pattern (60 BPM)
        let ecg = 2.0 * (2.0 * std::f64::consts::PI * 1.0 * t).sin();
        ecg_samples.push(ecg);
    }
    
    writer.write_samples(&[eeg_samples, ecg_samples])?;
    writer.finalize()?;
    Ok(())
}

/// Creates a test file with known digital value ranges for validation
pub fn create_validation_test_file<P: AsRef<Path>>(path: P) -> Result<()> {
    let mut writer = EdfWriter::create(&path)?;
    
    writer.set_patient_info("VAL001", "X", "X", "Validation Test")?;
    
    // Add signal with specific ranges for testing
    writer.add_signal(SignalParam {
        label: "Test Signal".to_string(),
        samples_in_file: 0,
        physical_max: 100.0,
        physical_min: -100.0,
        digital_max: 1000,
        digital_min: -1000,
        samples_per_record: 10,
        physical_dimension: "uV".to_string(),
        prefilter: "None".to_string(),
        transducer: "Test Sensor".to_string(),
    })?;
    
    // Generate predictable test data
    let samples = vec![100.0, 50.0, 0.0, -50.0, -100.0, 75.0, 25.0, -25.0, -75.0, 0.0];
    writer.write_samples(&[samples])?;
    writer.finalize()?;
    Ok(())
}

/// Cleanup function to remove test files after doctests
pub fn cleanup_doctest_files() {
    let test_files = [
        "recording.edf",
        "eeg_recording.edf", 
        "multi_channel.edf",
        "multi_signal.edf",
        "test.edf",
        "output.edf",
        "samples.edf",
        "continuous.edf",
        "patient_data.edf",
        "anonymous.edf",
        "study_subject.edf",
        "mixed_rates.edf",
        "new_recording.edf",
        "signals.edf",
        "final_test.edf",
        "error_test.edf",
    ];
    
    for file in &test_files {
        let _ = std::fs::remove_file(file);
    }
}
