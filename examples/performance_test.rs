use edfplus::{EdfWriter, EdfReader, SignalParam, Result};
use std::time::Instant;

fn main() -> Result<()> {
    println!("EDF+ 性能测试");
    println!("=============");
    
    // 创建一个较大的测试文件
    let file_path = "test_data/performance_test.edf";
    let sample_rate = 512; // 512 Hz
    let duration_seconds = 10; // 10秒数据
    let num_signals = 4; // 4个信号
    
    println!("创建测试文件: {} 秒, {} 信号, {} Hz", duration_seconds, num_signals, sample_rate);
    
    // 测试写入性能
    let write_start = Instant::now();
    {
        let mut writer = EdfWriter::create(file_path)?;
        writer.set_patient_info("PERF001", "X", "01-JAN-2000", "PerformanceTest")?;
        
        // 添加多个信号
        for i in 0..num_signals {
            let signal = SignalParam {
                label: format!("Signal{}", i + 1),
                samples_in_file: 0,
                physical_max: 100.0,
                physical_min: -100.0,
                digital_max: 32767,
                digital_min: -32768,
                samples_per_record: sample_rate,
                physical_dimension: "uV".to_string(),
                prefilter: "HP:0.1Hz LP:100Hz".to_string(),
                transducer: "Test electrode".to_string(),
            };
            writer.add_signal(signal)?;
        }
        
        // 生成并写入数据
        for second in 0..duration_seconds {
            let mut all_samples = Vec::new();
            
            for signal_idx in 0..num_signals {
                let mut signal_samples = Vec::new();
                
                for sample in 0..sample_rate {
                    let t = second as f64 + (sample as f64 / sample_rate as f64);
                    
                    // 为每个信号生成不同频率的正弦波
                    let frequency = 10.0 + signal_idx as f64 * 5.0; // 10, 15, 20, 25 Hz
                    let amplitude = 50.0 + signal_idx as f64 * 10.0; // 不同幅度
                    let value = amplitude * (2.0 * std::f64::consts::PI * frequency * t).sin();
                    
                    signal_samples.push(value);
                }
                
                all_samples.push(signal_samples);
            }
            
            writer.write_samples(&all_samples)?;
        }
        
        writer.finalize()?;
    }
    let write_duration = write_start.elapsed();
    
    // 计算文件大小
    let metadata = std::fs::metadata(file_path)?;
    let file_size_mb = metadata.len() as f64 / 1024.0 / 1024.0;
    
    println!("✅ 写入完成:");
    println!("  时间: {:.2} 秒", write_duration.as_secs_f64());
    println!("  文件大小: {:.2} MB", file_size_mb);
    println!("  写入速度: {:.2} MB/s", file_size_mb / write_duration.as_secs_f64());
    
    // 测试读取性能
    println!("\n📖 测试读取性能...");
    let read_start = Instant::now();
    
    let mut reader = EdfReader::open(file_path)?;
    let signals_info = {
        let header = reader.header();
        (
            header.signals.len(),
            header.signals[0].samples_in_file,
            header.file_duration,
            header.signals.clone()
        )
    };
    
    let (signals_len, total_samples, file_duration, signals) = signals_info;
    
    println!("文件信息:");
    println!("  信号数: {}", signals_len);
    println!("  总样本数: {}", total_samples);
    println!("  文件时长: {:.2} 秒", file_duration as f64 / 10_000_000.0);
    
    // 读取所有数据
    let mut total_samples_read = 0;
    for signal_idx in 0..signals.len() {
        let signal = &signals[signal_idx];
        let samples_to_read = signal.samples_in_file as usize;
        
        reader.rewind(signal_idx)?;
        let samples = reader.read_physical_samples(signal_idx, samples_to_read)?;
        total_samples_read += samples.len();
        
        // 验证数据质量 - 检查前几个样本
        if signal_idx == 0 {
            println!("  信号 {} 前5个样本: {:?}", 
                signal_idx, &samples[..5.min(samples.len())]);
        }
    }
    
    let read_duration = read_start.elapsed();
    
    println!("✅ 读取完成:");
    println!("  时间: {:.2} 秒", read_duration.as_secs_f64());
    println!("  总样本数: {}", total_samples_read);
    println!("  读取速度: {:.0} 样本/秒", total_samples_read as f64 / read_duration.as_secs_f64());
    println!("  数据速度: {:.2} MB/s", file_size_mb / read_duration.as_secs_f64());
    
    // 测试随机访问性能
    println!("\n🎯 测试随机访问性能...");
    let seek_start = Instant::now();
    
    let signal_idx = 0;
    let samples_in_file = signals[signal_idx].samples_in_file; // Use the cloned signals info
    let num_seeks = 100;
    
    for i in 0..num_seeks {
        let position = (i * samples_in_file as usize / num_seeks) as i64;
        reader.seek(signal_idx, position)?;
        let samples = reader.read_physical_samples(signal_idx, 10)?;
        
        if i == 0 {
            println!("  位置 {} 的样本: {:?}", position, &samples[..5.min(samples.len())]);
        }
    }
    
    let seek_duration = seek_start.elapsed();
    
    println!("✅ 随机访问完成:");
    println!("  操作次数: {}", num_seeks);
    println!("  总时间: {:.3} 秒", seek_duration.as_secs_f64());
    println!("  平均每次: {:.1} ms", seek_duration.as_millis() as f64 / num_seeks as f64);
    
    // 清理测试文件
    std::fs::remove_file(file_path).ok();
    
    println!("\n🏁 性能测试完成！");
    
    Ok(())
}
