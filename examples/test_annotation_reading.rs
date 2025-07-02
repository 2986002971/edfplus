use edfplus::{EdfWriter, EdfReader, SignalParam};
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_path = "test_annotation_reading.edf";
    
    // 清理旧文件
    let _ = fs::remove_file(file_path);
    
    println!("Creating EDF+ file with annotations...");
    
    // 创建一个EDF+文件并添加注释
    let annotation_count = {
        let mut writer = EdfWriter::create(file_path)?;
        
        // 设置文件信息
        writer.set_patient_info("TEST001", "M", "01-JAN-2000", "Test Patient")?;
        
        // 添加一个EEG信号
        let signal_param = SignalParam {
            label: "EEG1".to_string(),
            samples_in_file: 0, // 将自动计算
            physical_max: 100.0,
            physical_min: -100.0,
            digital_max: 32767,
            digital_min: -32768,
            samples_per_record: 100,
            physical_dimension: "uV".to_string(),
            prefilter: "HP:0.1Hz LP:70Hz".to_string(),
            transducer: "AgAgCl electrode".to_string(),
        };
        
        writer.add_signal(signal_param)?;
        writer.set_datarecord_duration(1.0)?; // 1秒每数据记录
        
        // 添加注释
        writer.add_annotation(1.5, Some(0.5), "Test annotation 1")?;
        writer.add_annotation(3.0, None, "Test annotation 2")?;
        writer.add_annotation(5.25, Some(1.0), "Test annotation 3 with longer duration")?;
        writer.add_annotation(7.8, None, "Final test annotation")?;
        
        // 写入10秒的模拟数据
        for record in 0..10 {
            let mut samples = Vec::new();
            
            // 为EEG1生成100个样本（正弦波）
            for i in 0..100 {
                let t = (record * 100 + i) as f64 * 0.01; // 时间（秒）
                let value = 50.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin(); // 10Hz正弦波
                samples.push(value);
            }
            
            writer.write_samples(&[samples])?;
        }
        
        let count = writer.annotation_count();
        writer.finalize()?;
        count
    };
    
    println!("Written {} annotations to file", annotation_count);
    
    println!("\nReading EDF+ file and parsing annotations...");
    
    // 读取文件并解析注释
    let reader = EdfReader::open(file_path)?;
    
    println!("File info:");
    println!("  Signals: {}", reader.header().signals.len());
    println!("  Data records: {}", reader.header().datarecords_in_file);
    println!("  Duration per record: {:.3}s", reader.header().datarecord_duration as f64 / 10000000.0);
    
    // 获取注释
    let annotations = reader.annotations();
    
    println!("\nFound {} annotations:", annotations.len());
    for (i, annotation) in annotations.iter().enumerate() {
        let onset_seconds = annotation.onset as f64 / 10000000.0;
        let duration_seconds = if annotation.duration >= 0 {
            annotation.duration as f64 / 10000000.0
        } else {
            -1.0
        };
        
        println!("  {}: onset={:.3}s, duration={:.3}s, text='{}'", 
                i + 1, 
                onset_seconds, 
                duration_seconds,
                annotation.description);
    }
    
    // 验证注释是否正确
    let expected_annotations = vec![
        (1.5, Some(0.5), "Test annotation 1"),
        (3.0, None, "Test annotation 2"),
        (5.25, Some(1.0), "Test annotation 3 with longer duration"),
        (7.8, None, "Final test annotation"),
    ];
    
    println!("\nValidation:");
    let mut all_correct = true;
    
    if annotations.len() != expected_annotations.len() {
        println!("  ERROR: Expected {} annotations, found {}", expected_annotations.len(), annotations.len());
        all_correct = false;
    } else {
        for (i, (expected_onset, expected_duration, expected_text)) in expected_annotations.iter().enumerate() {
            let annotation = &annotations[i];
            let onset_seconds = annotation.onset as f64 / 10000000.0;
            let duration_seconds = if annotation.duration >= 0 {
                Some(annotation.duration as f64 / 10000000.0)
            } else {
                None
            };
            
            let onset_ok = (onset_seconds - expected_onset).abs() < 0.001;
            let duration_ok = match (duration_seconds, expected_duration) {
                (Some(actual), Some(expected)) => (actual - expected).abs() < 0.001,
                (None, None) => true,
                _ => false,
            };
            let text_ok = annotation.description == *expected_text;
            
            if onset_ok && duration_ok && text_ok {
                println!("  ✓ Annotation {} correct", i + 1);
            } else {
                println!("  ✗ Annotation {} incorrect:", i + 1);
                if !onset_ok {
                    println!("    - Onset: expected {:.3}s, got {:.3}s", expected_onset, onset_seconds);
                }
                if !duration_ok {
                    println!("    - Duration: expected {:?}s, got {:?}s", expected_duration, duration_seconds);
                }
                if !text_ok {
                    println!("    - Text: expected '{}', got '{}'", expected_text, annotation.description);
                }
                all_correct = false;
            }
        }
    }
    
    if all_correct {
        println!("\n✓ All annotations parsed correctly!");
    } else {
        println!("\n✗ Some annotations were not parsed correctly.");
    }
    
    // 清理测试文件
    let _ = fs::remove_file(file_path);
    
    Ok(())
}
