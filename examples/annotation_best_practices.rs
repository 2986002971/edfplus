use edfplus::{EdfWriter, SignalParam};
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== EDF+ 注释使用最佳实践示例 ===");
    
    let filename = "annotation_best_practices.edf";
    
    // 清理旧文件
    fs::remove_file(filename).ok();
    
    println!("\n1. 创建EDF+文件并设置基本信息");
    let mut writer = EdfWriter::create(filename)?;
    writer.set_patient_info("DEMO001", "X", "01-JAN-2024", "Best Practices Demo")?;
    
    // 添加信号
    let signal = SignalParam {
        label: "EEG C3-A2".to_string(),
        samples_in_file: 0,
        physical_max: 100.0,
        physical_min: -100.0,
        digital_max: 32767,
        digital_min: -32768,
        samples_per_record: 100,  // 100 Hz
        physical_dimension: "uV".to_string(),
        prefilter: "0.1-35Hz".to_string(),
        transducer: "AgAgCl".to_string(),
    };
    writer.add_signal(signal)?;
    
    println!("2. 添加注释（在写入数据前）");
    
    // ✅ 好的做法
    println!("   ✅ 添加简洁的注释（≤40字符）");
    writer.add_annotation(5.0, None, "Start")?;
    writer.add_annotation(10.0, Some(5.0), "Event 1")?;
    writer.add_annotation(20.0, None, "Spindle")?;
    writer.add_annotation(30.0, None, "K-complex")?;
    writer.add_annotation(40.0, Some(10.0), "Stage N2")?;
    writer.add_annotation(55.0, None, "End")?;
    
    // ⚠️ 演示将被截断的长描述
    println!("   ⚠️  添加过长的描述（将被截断到40字符）");
    let long_description = "This is a very long annotation description that exceeds the 40 character limit and will be truncated";
    writer.add_annotation(25.0, None, long_description)?;
    
    println!("   ❌ 添加超出范围的注释（会被丢失）");
    // 这个注释将被丢失，因为超出了60秒的数据范围
    writer.add_annotation(65.0, None, "Beyond range")?;
    
    println!("3. 然后写入数据以建立有效的时间范围");
    let recording_duration_seconds = 60;  // 1分钟演示
    
    for second in 0..recording_duration_seconds {
        let mut samples = Vec::with_capacity(100);
        for sample_idx in 0..100 {
            let t = second as f64 + (sample_idx as f64 / 100.0);
            // 模拟简单的EEG信号
            let eeg_value = 20.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin() +
                           5.0 * (2.0 * std::f64::consts::PI * 2.0 * t).sin();
            samples.push(eeg_value);
        }
        writer.write_samples(&[samples])?;
    }
    
    println!("4. 完成文件");
    writer.finalize()?;
    
    println!("\n=== 验证结果 ===");
    
    // 读取并验证结果
    let reader = edfplus::EdfReader::open(filename)?;
    let annotations = reader.annotations();
    
    println!("文件信息:");
    println!("  总时长: {:.1}秒", reader.header().file_duration as f64 / 10_000_000.0);
    println!("  找到注释: {}个", annotations.len());
    
    println!("\n注释列表:");
    for (i, annotation) in annotations.iter().enumerate() {
        let onset_s = annotation.onset as f64 / 10_000_000.0;
        let duration_s = if annotation.duration >= 0 {
            Some(annotation.duration as f64 / 10_000_000.0)
        } else {
            None
        };
        
        let truncated = if annotation.description.len() == 40 && 
                          annotation.description.chars().last() != Some(' ') &&
                          !annotation.description.ends_with('.') {
            " [截断]"
        } else {
            ""
        };
        
        println!("  {}: {:.1}s - '{}'{} ({}字符, 持续时间: {:?})", 
                i + 1, 
                onset_s, 
                annotation.description,
                truncated,
                annotation.description.len(),
                duration_s
        );
    }
    
    println!("\n=== 最佳实践总结 ===");
    println!("✅ 做:");
    println!("  - 使用简洁的ASCII描述（≤40字符）");
    println!("  - 先写入数据，再添加注释");
    println!("  - 确保注释时间在数据范围内");
    println!("  - 使用标准的医学术语缩写");
    
    println!("\n❌ 不要:");
    println!("  - 使用超过40字符的描述");
    println!("  - 在写入数据前添加注释");
    println!("  - 添加超出文件时长的注释");
    println!("  - 使用复杂的UTF-8字符（可能被截断）");
    
    // 清理文件
    fs::remove_file(filename).ok();
    println!("\n演示完成，文件已清理");
    
    Ok(())
}
