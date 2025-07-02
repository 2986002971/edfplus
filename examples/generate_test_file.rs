use edfplus::{EdfWriter, SignalParam, Result};

fn main() -> Result<()> {
    println!("创建测试EDF+文件...");
    
    // 创建写入器
    let mut writer = EdfWriter::create("test_data/test_generated.edf")?;
    
    // 设置患者信息
    writer.set_patient_info("TEST001", "M", "01-JAN-1990", "TestPatient")?;
    
    // 创建两个测试信号
    let eeg_signal = SignalParam {
        label: "EEG Fp1".to_string(),
        samples_in_file: 1000, // 这会被自动计算
        physical_max: 200.0,   // +200 µV
        physical_min: -200.0,  // -200 µV
        digital_max: 32767,    // EDF最大值
        digital_min: -32768,   // EDF最小值
        samples_per_record: 256, // 256 Hz采样率
        physical_dimension: "uV".to_string(),
        prefilter: "HP:0.1Hz LP:70Hz".to_string(),
        transducer: "AgAgCl cup electrodes".to_string(),
    };
    
    let ecg_signal = SignalParam {
        label: "ECG Lead II".to_string(),
        samples_in_file: 1000,
        physical_max: 5.0,     // +5 mV
        physical_min: -5.0,    // -5 mV
        digital_max: 32767,
        digital_min: -32768,
        samples_per_record: 256, // 256 Hz采样率
        physical_dimension: "mV".to_string(),
        prefilter: "HP:0.05Hz LP:40Hz".to_string(),
        transducer: "Ag/AgCl electrodes".to_string(),
    };
    
    // 添加信号到写入器
    writer.add_signal(eeg_signal)?;
    writer.add_signal(ecg_signal)?;
    
    // 生成测试数据 (1秒的数据，256个样本)
    let mut eeg_samples = Vec::new();
    let mut ecg_samples = Vec::new();
    
    for i in 0..256 {
        let t = i as f64 / 256.0; // 时间 0-1秒
        
        // 生成模拟EEG信号: 10Hz正弦波 + 一些噪声
        let eeg_value = 50.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin() 
                      + 10.0 * (2.0 * std::f64::consts::PI * 30.0 * t).sin()
                      + 5.0 * (i as f64 % 7.0 - 3.0); // 模拟噪声
        eeg_samples.push(eeg_value);
        
        // 生成模拟ECG信号: 更复杂的波形
        let ecg_value = if (t * 60.0) % 1.0 < 0.1 { // 每秒一个R波
            2.0 * ((t * 60.0) % 1.0 * 20.0).sin()
        } else {
            0.1 * (2.0 * std::f64::consts::PI * 5.0 * t).sin()
        };
        ecg_samples.push(ecg_value);
    }
    
    // 写入样本数据
    let samples = vec![eeg_samples, ecg_samples];
    writer.write_samples(&samples)?;
    
    // 完成写入
    writer.finalize()?;
    
    println!("测试EDF+文件已生成: test_data/test_generated.edf");
    println!("包含 2 个信号，每个 256 个样本");
    
    Ok(())
}
