use edfplus::{EdfReader, EdfWriter, SignalParam};
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

// 清理测试文件的辅助函数
fn cleanup_test_file(filename: &str) {
    if Path::new(filename).exists() {
        fs::remove_file(filename).ok();
    }
}

// 创建测试信号的辅助函数
fn create_streaming_signal() -> SignalParam {
    SignalParam {
        label: "Stream Signal".to_string(),
        samples_in_file: 0,
        physical_max: 100.0,
        physical_min: -100.0,
        digital_max: 32767,
        digital_min: -32768,
        samples_per_record: 256,
        physical_dimension: "uV".to_string(),
        prefilter: "HP:0.1Hz LP:40Hz".to_string(),
        transducer: "Streaming electrodes".to_string(),
    }
}

#[test]
fn test_streaming_write_incremental_read() {
    let filename = "test_streaming.edf";
    
    // 模拟流式写入
    {
        let mut writer = EdfWriter::create(filename).unwrap();
        writer.set_patient_info("STREAM001", "X", "X", "Streaming Test").unwrap();
        
        let signal = create_streaming_signal();
        writer.add_signal(signal).unwrap();
        
        // 模拟实时数据流 - 每次写入1秒的数据
        for chunk in 0..30 {
            let mut samples = Vec::new();
            
            // 生成1秒的数据 (256样本)
            for i in 0..256 {
                let t = (chunk * 256 + i) as f64 / 256.0;
                // 模拟随时间变化的信号
                let base_freq = 10.0 + (chunk as f64 * 0.5); // 频率随时间增加
                let amplitude = 50.0 * (1.0 + 0.1 * (chunk as f64 * 0.2).sin()); // 振幅调制
                let value = amplitude * (2.0 * std::f64::consts::PI * base_freq * t).sin();
                samples.push(value);
            }
            
            writer.write_samples(&[samples]).unwrap();
            
            // 模拟实时延迟
            if chunk % 10 == 0 {
                println!("Streamed {} seconds of data", chunk + 1);
            }
        }
        
        writer.finalize().unwrap();
    }
    
    // 流式读取测试
    {
        let mut reader = EdfReader::open(filename).unwrap();
        let header = reader.header();
        
        println!("Streaming read test:");
        println!("  Total duration: {:.1}s", header.file_duration as f64 / 10_000_000.0);
        
        // 模拟逐段读取
        let chunk_size = 512; // 2秒的数据
        let mut total_samples_read = 0;
        let mut chunk_count = 0;
        
        loop {
            let samples = reader.read_physical_samples(0, chunk_size).unwrap();
            if samples.is_empty() {
                break;
            }
            
            total_samples_read += samples.len();
            chunk_count += 1;
            
            // 分析当前块的数据
            let mean = samples.iter().sum::<f64>() / samples.len() as f64;
            let max_val = samples.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            let min_val = samples.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            
            if chunk_count % 5 == 0 {
                let time_s = total_samples_read as f64 / 256.0;
                println!("    Chunk {}: {:.1}s, {} samples, range: {:.1} to {:.1}, mean: {:.2}",
                        chunk_count, time_s, samples.len(), min_val, max_val, mean);
            }
            
            // 模拟处理延迟
            thread::sleep(Duration::from_millis(1));
        }
        
        println!("  Total chunks read: {}", chunk_count);
        println!("  Total samples read: {}", total_samples_read);
        assert_eq!(total_samples_read, 30 * 256); // 30秒 * 256样本/秒
    }
    
    cleanup_test_file(filename);
}

#[test]
fn test_random_access_streaming() {
    let filename = "test_random_streaming.edf";
    
    // 创建长时间序列数据
    {
        let mut writer = EdfWriter::create(filename).unwrap();
        writer.set_patient_info("RANDOM001", "X", "X", "Random Access Test").unwrap();
        
        let signal = create_streaming_signal();
        writer.add_signal(signal).unwrap();
        
        // 写入5分钟的数据，包含明显的时间模式
        for second in 0..300 {
            let mut samples = Vec::new();
            for i in 0..256 {
                let t = (second * 256 + i) as f64 / 256.0;
                
                // 创建可识别的时间模式
                let minute = second / 60;
                let freq = 5.0 + minute as f64 * 2.0; // 每分钟频率增加2Hz
                let phase_shift = minute as f64 * std::f64::consts::PI / 4.0; // 每分钟相位偏移
                
                let value = 40.0 * (2.0 * std::f64::consts::PI * freq * t + phase_shift).sin();
                samples.push(value);
            }
            writer.write_samples(&[samples]).unwrap();
        }
        
        writer.finalize().unwrap();
    }
    
    // 随机访问测试
    {
        let mut reader = EdfReader::open(filename).unwrap();
        
        println!("Random access streaming test:");
        
        // 测试随机跳转到不同时间点
        let test_positions = vec![
            (0, "Start"),           // 开始
            (60, "1 minute"),       // 1分钟
            (150, "2.5 minutes"),   // 2.5分钟
            (240, "4 minutes"),     // 4分钟
            (290, "Near end"),      // 接近结束
        ];
        
        for (target_second, description) in test_positions {
            let target_sample = target_second * 256;
            
            // 跳转到目标位置
            reader.seek(0, target_sample).unwrap();
            
            // 读取该位置的数据
            let samples = reader.read_physical_samples(0, 256).unwrap();
            assert_eq!(samples.len(), 256);
            
            // 分析频率特征以验证位置正确性
            let current_pos = reader.tell(0).unwrap();
            let actual_second = (current_pos - 256) / 256; // 减去刚读取的256样本
            
            // 计算主要频率成分（简单FFT替代）
            let mut freq_powers = vec![0.0; 20]; // 检测0-20Hz
            for freq_idx in 0..20 {
                let freq = freq_idx as f64;
                let mut power = 0.0;
                for (i, &sample) in samples.iter().enumerate() {
                    let t = i as f64 / 256.0;
                    power += sample * (2.0 * std::f64::consts::PI * freq * t).cos();
                }
                freq_powers[freq_idx] = power.abs();
            }
            
            // 找到最强的频率
            let dominant_freq = freq_powers.iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap().0 as f64;
            
            println!("    {}: sample {}, dominant freq: {:.1}Hz, expected ~{:.1}Hz",
                    description, actual_second, dominant_freq, 
                    5.0 + (target_second / 60) as f64 * 2.0);
        }
        
        // 测试顺序读取vs随机访问的一致性
        reader.rewind(0).unwrap();
        let sequential_start = reader.read_physical_samples(0, 256).unwrap();
        
        reader.seek(0, 0).unwrap();
        let random_start = reader.read_physical_samples(0, 256).unwrap();
        
        // 验证两种读取方式结果一致
        assert_eq!(sequential_start.len(), random_start.len());
        for (i, (&seq, &rand)) in sequential_start.iter().zip(random_start.iter()).enumerate() {
            let diff = (seq - rand).abs();
            assert!(diff < 1e-10, "Mismatch at sample {}: sequential={}, random={}", i, seq, rand);
        }
        
        println!("    Sequential vs random access: consistent ✓");
    }
    
    cleanup_test_file(filename);
}

#[test]
fn test_large_file_handling() {
    let filename = "test_large_file.edf";
    
    // 创建相对较大的文件（10分钟，多通道）
    {
        let mut writer = EdfWriter::create(filename).unwrap();
        writer.set_patient_info("LARGE001", "X", "X", "Large File Test").unwrap();
        
        // 添加8个通道模拟密集的EEG记录
        for i in 0..8 {
            let mut signal = create_streaming_signal();
            signal.label = format!("EEG_{}", i + 1);
            writer.add_signal(signal).unwrap();
        }
        
        println!("Creating large file with 8 channels for 10 minutes...");
        
        // 写入10分钟的数据
        for second in 0..600 {
            let mut all_samples = Vec::new();
            
            for channel in 0..8 {
                let mut channel_samples = Vec::new();
                for sample in 0..256 {
                    let t = (second * 256 + sample) as f64 / 256.0;
                    let freq = 8.0 + channel as f64; // 每个通道不同频率
                    let amplitude = 30.0 + channel as f64 * 5.0; // 每个通道不同幅度
                    let value = amplitude * (2.0 * std::f64::consts::PI * freq * t).sin();
                    channel_samples.push(value);
                }
                all_samples.push(channel_samples);
            }
            
            writer.write_samples(&all_samples).unwrap();
            
            if second % 60 == 0 {
                println!("  Written {} minutes", second / 60);
            }
        }
        
        writer.finalize().unwrap();
    }
    
    // 大文件读取性能测试
    {
        let start_time = std::time::Instant::now();
        let mut reader = EdfReader::open(filename).unwrap();
        let open_time = start_time.elapsed();
        
        let header = reader.header();
        println!("Large file test results:");
        println!("  File open time: {:.2}ms", open_time.as_millis());
        println!("  Channels: {}", header.signals.len());
        println!("  Duration: {:.1} minutes", header.file_duration as f64 / 10_000_000.0 / 60.0);
        
        // 测试随机访问性能
        let seek_start = std::time::Instant::now();
        
        // 跳转到文件中间
        let middle_position = 300 * 256; // 5分钟位置
        reader.seek(0, middle_position).unwrap();
        
        let seek_time = seek_start.elapsed();
        println!("  Seek to middle: {:.2}ms", seek_time.as_millis());
        
        // 测试批量读取性能
        let read_start = std::time::Instant::now();
        
        let mut total_samples = 0;
        for channel in 0..8 {
            let samples = reader.read_physical_samples(channel, 2560).unwrap(); // 10秒数据
            total_samples += samples.len();
        }
        
        let read_time = read_start.elapsed();
        println!("  Bulk read (8 channels × 10s): {:.2}ms for {} samples", 
                read_time.as_millis(), total_samples);
        println!("  Read rate: {:.1} samples/ms", total_samples as f64 / read_time.as_millis() as f64);
        
        // 验证数据完整性
        reader.rewind(0).unwrap();
        let first_samples = reader.read_physical_samples(0, 256).unwrap();
        assert_eq!(first_samples.len(), 256);
        
        // 跳到最后检查
        let final_position = 599 * 256; // 最后一秒
        reader.seek(0, final_position).unwrap();
        let final_samples = reader.read_physical_samples(0, 256).unwrap();
        assert_eq!(final_samples.len(), 256);
        
        println!("  Data integrity: ✓");
    }
    
    cleanup_test_file(filename);
}

#[test]
fn test_concurrent_read_access() {
    let filename = "test_concurrent.edf";
    
    // 创建测试文件
    {
        let mut writer = EdfWriter::create(filename).unwrap();
        writer.set_patient_info("CONC001", "X", "X", "Concurrent Test").unwrap();
        
        let signal = create_streaming_signal();
        writer.add_signal(signal).unwrap();
        
        // 写入60秒的测试数据
        for second in 0..60 {
            let mut samples = Vec::new();
            for i in 0..256 {
                let t = (second * 256 + i) as f64 / 256.0;
                let value = 30.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin() +
                           (second as f64).sin() * 10.0; // 添加慢变化成分用于验证
                samples.push(value);
            }
            writer.write_samples(&[samples]).unwrap();
        }
        
        writer.finalize().unwrap();
    }
    
    // 测试多个读取器同时访问同一文件
    {
        use std::sync::{Arc, Mutex};
        use std::thread;
        
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        
        println!("Testing concurrent file access with 4 readers...");
        
        for reader_id in 0..4 {
            let results_clone = Arc::clone(&results);
            let filename_clone = filename.to_string();
            
            let handle = thread::spawn(move || {
                let mut reader = EdfReader::open(&filename_clone).unwrap();
                let mut reader_results = Vec::new();
                
                // 每个读取器读取不同的时间段
                let start_second = reader_id * 15; // 0, 15, 30, 45秒开始
                let start_sample = start_second * 256;
                
                reader.seek(0, start_sample).unwrap();
                
                // 读取15秒的数据
                for _ in 0..15 {
                    let samples = reader.read_physical_samples(0, 256).unwrap();
                    if !samples.is_empty() {
                        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
                        reader_results.push(mean);
                    }
                }
                
                let mut results_lock = results_clone.lock().unwrap();
                results_lock.push((reader_id, reader_results));
            });
            
            handles.push(handle);
        }
        
        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }
        
        // 验证结果
        let results_lock = results.lock().unwrap();
        assert_eq!(results_lock.len(), 4);
        
        for (reader_id, reader_results) in results_lock.iter() {
            assert_eq!(reader_results.len(), 15);
            println!("  Reader {}: read {} segments, first mean: {:.3}", 
                    reader_id, reader_results.len(), reader_results[0]);
        }
        
        println!("  Concurrent access: ✓");
    }
    
    cleanup_test_file(filename);
}

#[test]
fn test_streaming_with_annotations() {
    let filename = "test_streaming_annotations.edf";
    
    // 创建包含注释的流式数据
    {
        let mut writer = EdfWriter::create(filename).unwrap();
        writer.set_patient_info("STREAM_ANN001", "X", "X", "Streaming with Annotations").unwrap();
        
        let signal = create_streaming_signal();
        writer.add_signal(signal).unwrap();
        
        // 预先添加注释（模拟已知的事件时间）
        writer.add_annotation(5.0, None, "Event 1").unwrap();
        writer.add_annotation(15.5, Some(2.0), "Long Event").unwrap();
        writer.add_annotation(25.2, None, "Event 2").unwrap();
        writer.add_annotation(35.7, Some(1.5), "Another Long Event").unwrap();
        writer.add_annotation(45.1, None, "Final Event").unwrap();
        
        // 写入50秒的数据
        for second in 0..50 {
            let mut samples = Vec::new();
            for i in 0..256 {
                let t = (second * 256 + i) as f64 / 256.0;
                let value = 25.0 * (2.0 * std::f64::consts::PI * 12.0 * t).sin();
                samples.push(value);
            }
            writer.write_samples(&[samples]).unwrap();
        }
        
        writer.finalize().unwrap();
    }
    
    // 流式读取并处理注释
    {
        let mut reader = EdfReader::open(filename).unwrap();
        let annotations = reader.annotations().to_vec();
        
        println!("Streaming with annotations test:");
        println!("  Found {} annotations", annotations.len());
        
        // 按时间窗口读取数据，同时关注注释
        let window_size = 256 * 5; // 5秒窗口
        let mut current_position = 0i64;
        let mut window_count = 0;
        
        loop {
            reader.seek(0, current_position).unwrap();
            let samples = reader.read_physical_samples(0, window_size).unwrap();
            
            if samples.is_empty() {
                break;
            }
            
            let window_start_time = current_position as f64 / 256.0;
            let window_end_time = window_start_time + samples.len() as f64 / 256.0;
            
            // 查找当前窗口内的注释
            let window_annotations: Vec<_> = annotations.iter()
                .filter(|ann| {
                    let ann_time = ann.onset as f64 / 10_000_000.0;
                    ann_time >= window_start_time && ann_time < window_end_time
                })
                .collect();
            
            // 分析窗口数据
            let _mean = samples.iter().sum::<f64>() / samples.len() as f64;
            let max_val = samples.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            let min_val = samples.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            
            println!("  Window {}: {:.1}-{:.1}s, {} samples, range: {:.1} to {:.1}",
                    window_count, window_start_time, window_end_time, 
                    samples.len(), min_val, max_val);
            
            // 报告窗口内的注释
            for annotation in window_annotations {
                let ann_time = annotation.onset as f64 / 10_000_000.0;
                let relative_time = ann_time - window_start_time;
                println!("    Annotation at +{:.1}s: {}", relative_time, annotation.description);
            }
            
            current_position += samples.len() as i64;
            window_count += 1;
        }
        
        println!("  Processed {} windows", window_count);
        
        // 验证所有注释都被发现
        let expected_count = 5;
        assert_eq!(annotations.len(), expected_count);
    }
    
    cleanup_test_file(filename);
}
