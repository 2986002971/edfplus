use edfplus::{EdfWriter, SignalParam, Result};

/// 简单的线性同余随机数生成器
/// 用于生成模拟信号，避免外部依赖
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new() -> Self {
        Self { state: 12345 }
    }
    
    fn next_f64(&mut self) -> f64 {
        self.state = self.state.wrapping_mul(1103515245).wrapping_add(12345);
        (self.state as f64) / (u64::MAX as f64)
    }
}

/// 演示如何创建多通道EEG文件
/// 这个示例创建了一个包含8个通道的EEG记录：
/// - 6个EEG通道 (Fp1, Fp2, C3, C4, O1, O2)
/// - 1个EOG通道 (眼电图)
/// - 1个EMG通道 (肌电图)
fn main() -> Result<()> {
    println!("🧠 创建多通道EEG记录文件...");
    
    // 创建写入器
    let mut writer = EdfWriter::create("multi_channel_eeg.edf")?;
    
    // 设置患者信息
    writer.set_patient_info("P001", "M", "01-JAN-1990", "Multi-channel EEG Study")?;
    
    // 定义多个EEG通道
    let channels = vec![
        ("EEG Fp1", -200.0, 200.0),  // 前额左
        ("EEG Fp2", -200.0, 200.0),  // 前额右  
        ("EEG C3", -200.0, 200.0),   // 中央左
        ("EEG C4", -200.0, 200.0),   // 中央右
        ("EEG O1", -200.0, 200.0),   // 枕部左
        ("EEG O2", -200.0, 200.0),   // 枕部右
        ("EOG", -500.0, 500.0),      // 眼电图
        ("EMG", -100.0, 100.0),      // 肌电图
    ];
    
    println!("📊 添加 {} 个信号通道...", channels.len());
    
    // 为每个通道添加信号参数
    for (label, phys_min, phys_max) in &channels {
        let signal = SignalParam {
            label: label.to_string(),
            samples_in_file: 0,
            physical_max: *phys_max,
            physical_min: *phys_min,
            digital_max: 32767,
            digital_min: -32768,
            samples_per_record: 256,  // 256 Hz采样率
            physical_dimension: "uV".to_string(),
            prefilter: "HP:0.1Hz LP:70Hz".to_string(),
            transducer: "AgAgCl cup electrodes".to_string(),
        };
        writer.add_signal(signal)?;
        println!("  ✓ 添加通道: {} (范围: {:.1} 到 {:.1} μV)", label, phys_min, phys_max);
    }
    
    // 添加一些实验事件注释
    println!("📝 添加实验事件注释...");
    writer.add_annotation(0.0, None, "Recording start")?;
    writer.add_annotation(3.5, None, "Attention task begin")?;
    writer.add_annotation(7.2, None, "Task end, rest begins")?;
    writer.add_annotation(9.8, None, "Recording end")?;
    
    // 模拟记录10秒的数据（10个数据记录，每个1秒）
    println!("🎥 记录 10 秒的模拟EEG数据...");
    
    let mut rng = SimpleRng::new();
    
    for record in 0..10 {
        let mut all_samples = Vec::new();
        
        // 为每个通道生成一秒的数据（256个样本）
        for (chan_idx, (label, _, _)) in channels.iter().enumerate() {
            let mut channel_samples = Vec::new();
            
            for i in 0..256 {
                let t = (record as f64) + (i as f64 / 256.0);
                
                // 根据通道类型生成不同的信号
                let value = match *label {
                    label if label.starts_with("EEG") => {
                        // EEG信号：多个频率成分的组合
                        let alpha = 20.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin();
                        let beta = 5.0 * (2.0 * std::f64::consts::PI * 20.0 * t).sin();
                        let delta = 10.0 * (2.0 * std::f64::consts::PI * 2.0 * t).sin();
                        let noise = (rng.next_f64() - 0.5) * 8.0;
                        
                        // 根据通道位置添加轻微的相位差
                        let phase_offset = chan_idx as f64 * 0.1;
                        let alpha_mod = alpha * (t * 0.1 + phase_offset).cos();
                        
                        alpha_mod + beta + delta + noise
                    },
                    "EOG" => {
                        // 眼电图：模拟眨眼信号
                        let blink_freq = 0.3; // 每3秒左右眨一次眼
                        let blink_amplitude = if (t * blink_freq).sin() > 0.8 { 150.0 } else { 0.0 };
                        let slow_drift = 20.0 * (0.05 * t).sin();
                        let noise = (rng.next_f64() - 0.5) * 15.0;
                        blink_amplitude + slow_drift + noise
                    },
                    "EMG" => {
                        // 肌电图：高频肌肉活动
                        let base_activity = (rng.next_f64() - 0.5) * 30.0;
                        let tension_cycle = 1.0 + 0.5 * (t * 0.2).sin(); // 周期性肌肉紧张
                        base_activity * tension_cycle
                    },
                    _ => 0.0
                };
                
                channel_samples.push(value);
            }
            all_samples.push(channel_samples);
        }
        
        // 写入所有通道的数据
        writer.write_samples(&all_samples)?;
        
        // 显示进度
        if (record + 1) % 2 == 0 {
            println!("  ⏱️  已记录: {} 秒", record + 1);
        }
    }
    
    writer.finalize()?;
    
    println!("✅ 多通道EEG文件创建完成！");
    println!();
    println!("📋 文件信息:");
    println!("  • 文件名: multi_channel_eeg.edf");
    println!("  • 通道数: {}", channels.len());
    println!("  • 记录时长: 10 秒");
    println!("  • 采样率: 256 Hz");
    println!("  • 总样本数: {} (每通道)", 10 * 256);
    println!("  • 估计文件大小: ~{} KB", (10 * channels.len() * 256 * 2) / 1024);
    println!();
    println!("💡 提示: 可以使用任何EDF+兼容的软件打开这个文件，如:");
    println!("    - EDFbrowser");
    println!("    - EEGLAB");
    println!("    - MNE-Python");
    println!("    - 或者使用本库的读取功能");
    
    Ok(())
}
