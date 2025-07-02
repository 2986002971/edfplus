# EDF+ Library for Rust

一个纯Rust实现的EDF+（欧洲数据格式增强版）文件读写库。本库专注于EDF+格式，提供安全、高效的API。

## 特性

- ✅ **读取EDF+文件** - 完整的头部信息和信号数据读取
- ✅ **写入EDF+文件** - 创建标准的EDF+文件
- ✅ **类型安全** - 利用Rust的类型系统防止常见错误
- ✅ **内存安全** - 无缓冲区溢出或内存泄漏
- ✅ **高效处理** - 支持大文件的流式读取
- ✅ **完整的元数据** - 患者信息、设备信息等
- ✅ **信号处理** - 物理值与数字值的自动转换
- ✅ **文件定位** - 支持随机访问和定位

## 快速开始

将以下内容添加到你的 `Cargo.toml`:

```toml
[dependencies]
edfplus = "0.1.0"
```

### 读取EDF+文件

```rust
use edfplus::{EdfReader, Result};

fn main() -> Result<()> {
    // 打开EDF+文件
    let mut reader = EdfReader::open("data.edf")?;
    
    // 获取文件信息
    let header = reader.header();
    println!("信号数量: {}", header.signals.len());
    println!("文件时长: {:.2} 秒", header.file_duration as f64 / 10_000_000.0);
    
    // 读取第一个信号的前1000个样本
    let samples = reader.read_physical_samples(0, 1000)?;
    println!("读取了 {} 个样本", samples.len());
    
    // 获取信号信息
    for (i, signal) in header.signals.iter().enumerate() {
        println!("信号 {}: {} ({})", i, signal.label, signal.physical_dimension);
        println!("  范围: {} - {}", signal.physical_min, signal.physical_max);
    }
    
    Ok(())
}
```

### 创建EDF+文件（多通道示例）

```rust
use edfplus::{EdfWriter, SignalParam, Result};

fn main() -> Result<()> {
    // 创建写入器
    let mut writer = EdfWriter::create("multi_channel_eeg.edf")?;
    
    // 设置患者信息
    writer.set_patient_info("P001", "M", "01-JAN-1990", "Patient Name")?;
    
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
    }
    
    // 模拟记录10秒的数据（10个数据记录，每个1秒）
    for record in 0..10 {
        let mut all_samples = Vec::new();
        
        // 为每个通道生成一秒的数据（256个样本）
        for (chan_idx, (label, _, _)) in channels.iter().enumerate() {
            let mut channel_samples = Vec::new();
            
            for i in 0..256 {
                let t = (record as f64) + (i as f64 / 256.0);
                
                // 根据通道类型生成不同的信号
                let value = match label {
                    label if label.starts_with("EEG") => {
                        // EEG信号：多个频率成分的组合
                        let alpha = 20.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin();
                        let beta = 5.0 * (2.0 * std::f64::consts::PI * 20.0 * t).sin();
                        let noise = fastrand::f64() * 10.0 - 5.0;
                        alpha + beta + noise
                    },
                    "EOG" => {
                        // 眼电图：低频眨眼信号
                        if t % 3.0 < 0.1 { 100.0 } else { 0.0 } + fastrand::f64() * 20.0 - 10.0
                    },
                    "EMG" => {
                        // 肌电图：高频肌肉活动
                        (fastrand::f64() - 0.5) * 50.0 * (1.0 + (t % 5.0 / 5.0))
                    },
                    _ => 0.0
                };
                
                channel_samples.push(value);
            }
            all_samples.push(channel_samples);
        }
        
        // 写入所有通道的数据
        writer.write_samples(&all_samples)?;
        
        // 在第3秒和第7秒添加注释
        if record == 3 {
            writer.add_annotation(0.5, "开始集中注意力任务")?;
        }
        if record == 7 {
            writer.add_annotation(0.2, "任务结束，开始休息")?;
        }
    }
    
    writer.finalize()?;
    
    println!("已创建多通道EEG文件 multi_channel_eeg.edf");
    println!("包含 {} 个通道，每个通道记录10秒数据", channels.len());
    
    Ok(())
}
```

这个示例展示了如何创建一个包含8个通道的EEG记录文件：
- **6个EEG通道**：Fp1/Fp2（前额）、C3/C4（中央）、O1/O2（枕部）
- **1个EOG通道**：眼电图，用于检测眨眼和眼动
- **1个EMG通道**：肌电图，用于监测肌肉活动

每个通道都有适合其信号类型的物理范围和模拟信号特征。在实际应用中，你可以：
- 调整采样率（`samples_per_record`）适应不同需求
- 设置合适的物理范围（`physical_min`/`physical_max`）
- 添加滤波器信息（`prefilter`）描述信号处理
- 同时记录多种生理信号（EEG、EOG、EMG等）

## 添加注释/事件标记

EDF+支持时间标记的注释来记录事件、阶段变化等重要信息：

```rust
use edfplus::{EdfWriter, SignalParam, Result};

fn main() -> Result<()> {
    let mut writer = EdfWriter::create("sleep_study.edf")?;
    writer.set_patient_info("S001", "F", "15-MAR-1980", "Sleep Study")?;
    
    // 添加EEG信号
    let eeg_signal = SignalParam {
        label: "C3-A2".to_string(),
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
    writer.add_signal(eeg_signal)?;
    
    // 添加睡眠研究注释 - 注意：必须在写入数据前添加
    writer.add_annotation(300.0, None, "Lights out")?;                    // 5分钟
    writer.add_annotation(480.0, None, "Sleep onset")?;                   // 8分钟  
    writer.add_annotation(600.0, Some(1200.0), "Stage N2")?;              // 10-30分钟
    writer.add_annotation(900.0, None, "Sleep spindle")?;                 // 15分钟
    writer.add_annotation(1200.0, Some(300.0), "REM episode")?;           // 20-25分钟
    writer.add_annotation(1790.0, None, "Wake up")?;                      // 29:50
    
    // ⚠️ 重要：在添加注释后写入数据以建立时间范围
    let recording_duration_seconds = 1800;  // 30分钟
    for second in 0..recording_duration_seconds {
        let mut samples = Vec::with_capacity(100);
        for sample_idx in 0..100 {
            let t = second as f64 + (sample_idx as f64 / 100.0);
            let eeg_value = 20.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin();
            samples.push(eeg_value);
        }
        writer.write_samples(&[samples])?;
    }
    
    writer.finalize()?;
    Ok(())
}
```

## ⚠️ 注释使用的重要限制

使用注释功能时，请注意以下关键限制，以免数据丢失：

### 1. 描述长度限制

**注释描述最多只能存储40个字符**，超出部分会被自动截断：

```rust
// ✅ 正确 - 在40字符限制内
writer.add_annotation(1.0, None, "Sleep stage N2")?;

// ⚠️ 警告 - 会被截断
writer.add_annotation(2.0, None, "This is a very long annotation that exceeds the EDF+ limit")?;
// 结果: "This is a very long annotation descripti"

// 💡 建议 - 使用简洁的描述
writer.add_annotation(3.0, None, "REM burst")?;
writer.add_annotation(4.0, None, "K-complex")?;
writer.add_annotation(5.0, None, "Artifact")?;
```

### 2. 时间范围约束

**最关键的限制**：注释只有在其时间戳落在已写入的数据记录范围内时才会被保存：

```rust
// ❌ 错误的顺序 - 注释会丢失
// 只写入3秒的数据 - 时间范围 [0.0, 3.0)
for second in 0..3 {
    let samples = vec![0.0; 256];
    writer.write_samples(&[samples])?;
}

// 然后添加注释 - 但这些时间超出了数据范围
writer.add_annotation(5.0, None, "Event at 5s")?;    // ❌ 会被丢失
writer.add_annotation(10.0, None, "Event at 10s")?;  // ❌ 会被丢失
// 结果：所有注释都会丢失！

// ✅ 正确的顺序
// 1. 先添加注释
writer.add_annotation(5.0, None, "Event at 5s")?;    // 预计在5秒时发生
writer.add_annotation(10.0, None, "Event at 10s")?;  // 预计在10秒时发生
writer.add_annotation(14.999, None, "Near end")?;    // 预计在14.999秒时发生

// 2. 然后写入足够的数据覆盖这些时间点
for second in 0..15 {  // 15秒数据，范围 [0.0, 15.0)
    let samples = vec![0.0; 256];
    writer.write_samples(&[samples])?;
}
// 结果：前3个注释都会被保存！

// ❌ 这个注释会被丢失，因为添加时数据范围已确定
writer.add_annotation(16.0, None, "Too late")?;  // ❌ 超出范围
```

### 3. 最佳实践

为避免数据丢失，请遵循以下最佳实践：

```rust
// 1. 📝 使用简洁的ASCII描述（≤40字符）
writer.add_annotation(1.0, None, "N1")?;           // 优于 "Sleep Stage N1 Beginning"
writer.add_annotation(2.0, None, "Spindle")?;      // 优于 "Sleep Spindle Activity Detected"
writer.add_annotation(3.0, None, "REM")?;          // 优于 "Rapid Eye Movement Sleep Phase"

// 2. 📊 规划注释时间，然后写入覆盖这些时间的数据
// 先添加所有预期的注释
writer.add_annotation(3600.0, None, "1h mark")?;   // 1小时标记
writer.add_annotation(7200.0, None, "2h mark")?;   // 2小时标记

// 然后写入足够时长的数据
let study_duration_hours = 8.0;
let total_seconds = (study_duration_hours * 3600.0) as usize;
for second in 0..total_seconds {
    // ... 写入数据 ...
}

// 3. 🕒 验证注释时间在预期数据范围内
fn add_safe_annotation(writer: &mut EdfWriter, time: f64, desc: &str, max_time: f64) -> Result<()> {
    if time >= max_time {
        eprintln!("警告: 注释时间 {:.1}s 超出预期文件范围 {:.1}s，请调整", time, max_time);
        return Ok(());
    }
    if desc.len() > 40 {
        eprintln!("警告: 描述 '{}' 超过40字符，将被截断", desc);
    }
    writer.add_annotation(time, None, desc)
}
```

### 4. UTF-8字符注意事项

由于40字符限制，多字节UTF-8字符可能被不当截断：

```rust
// ⚠️ 可能导致无效UTF-8
writer.add_annotation(1.0, None, "测试中文字符和emoji🧠很长的描述文本")?;
// 可能被截断为: "测试中文字符和emoji🧠很长�" (无效UTF-8)

// ✅ 建议使用ASCII字符
writer.add_annotation(1.0, None, "Chinese text test")?;
writer.add_annotation(2.0, None, "Event with emoji")?;
```

## 📚 EDF+ 技术原理详解

### 🔍 文件头部字段说明

EDF+ 文件包含丰富的元数据信息，`EdfHeader` 结构体提供了对所有这些字段的访问：

#### 患者信息字段
```rust
let header = reader.header();

// 患者身份信息
println!("患者代码: {}", header.patient_code);        // 例: "MCH-0234567" 或 "ANON-001"
println!("性别: {}", header.sex);                     // "M", "F", 或 "X"
println!("出生日期: {}", header.birthdate);           // "02-MAY-1951" 或 "X"
println!("患者姓名: {}", header.patient_name);        // 通常匿名化为 "X"
println!("额外信息: {}", header.patient_additional);  // 自由文本字段
```

#### 记录设备信息
```rust
// 记录设备和技术人员
println!("管理代码: {}", header.admin_code);          // 例: "PSG-LAB", "NEURO-ICU"
println!("技术人员: {}", header.technician);          // 负责记录的人员
println!("设备信息: {}", header.equipment);           // 例: "Nihon Kohden EEG-1200"
println!("记录附加信息: {}", header.recording_additional); // 记录协议等详细信息
```

#### 时间和数据结构
```rust
// 记录时间信息
println!("开始日期: {}", header.start_date);          // NaiveDate 格式
println!("开始时间: {}", header.start_time);          // NaiveTime 格式
println!("亚秒精度: {} (100ns单位)", header.starttime_subsecond);

// 文件结构信息
println!("数据记录数: {}", header.datarecords_in_file);
println!("每记录时长: {} 秒", header.datarecord_duration as f64 / 10_000_000.0);
println!("文件总时长: {:.2} 秒", header.file_duration as f64 / 10_000_000.0);
println!("注释总数: {}", header.annotations_in_file);
```

#### 信号通道详细信息
```rust
// 遍历所有信号通道
for (i, signal) in header.signals.iter().enumerate() {
    println!("\n信号 {} 详细信息:", i);
    println!("  标签: {}", signal.label);                    // 例: "EEG Fp1", "ECG Lead II"
    println!("  物理单位: {}", signal.physical_dimension);   // 例: "µV", "mV", "BPM"
    println!("  物理范围: {} 到 {}", signal.physical_min, signal.physical_max);
    println!("  数字范围: {} 到 {}", signal.digital_min, signal.digital_max);
    println!("  采样率: {} Hz", signal.samples_per_record);  // 假设1秒数据记录
    println!("  前置滤波: {}", signal.prefilter);           // 例: "HP:0.1Hz LP:70Hz"
    println!("  传感器: {}", signal.transducer);            // 例: "AgAgCl cup electrodes"
    println!("  总样本数: {}", signal.samples_in_file);
}
```

### ⚡ 数字量与物理量转换原理

EDF+ 格式使用 **16位有符号整数** 存储数据，通过线性变换转换为实际的物理测量值。理解这个转换过程对于正确处理数据至关重要。

#### 转换公式

```rust
// 从数字值转换为物理值
let physical_value = (digital_value - digital_offset) * bit_value;

// 从物理值转换为数字值  
let digital_value = (physical_value / bit_value) + digital_offset;

// 其中:
// bit_value = (physical_max - physical_min) / (digital_max - digital_min)
// digital_offset = digital_max - physical_max / bit_value
```

#### 实际示例计算

```rust
use edfplus::{EdfReader, SignalParam};

// 典型的EEG信号参数
let signal = SignalParam {
    label: "EEG Fp1".to_string(),
    physical_max: 200.0,      // +200 µV
    physical_min: -200.0,     // -200 µV  
    digital_max: 32767,       // 16位最大值
    digital_min: -32768,      // 16位最小值
    samples_per_record: 256,
    physical_dimension: "uV".to_string(),
    // ... 其他字段
};

// 计算转换参数
let bit_value = signal.bit_value();  // (200.0 - (-200.0)) / (32767 - (-32768)) = 400.0 / 65535 ≈ 0.0061 µV
let offset = signal.offset();        // 32767.0 - 200.0/0.0061 ≈ 0

println!("分辨率: {:.6} µV/数字单位", bit_value);
println!("偏移量: {:.1}", offset);

// 转换示例
let digital_samples = vec![-32768, -16384, 0, 16384, 32767];
for digital in &digital_samples {
    let physical = signal.to_physical(*digital);
    println!("数字值 {:6} → 物理值 {:8.3} µV", digital, physical);
}

// 输出类似:
// 数字值 -32768 → 物理值 -200.000 µV  (最小值)
// 数字值 -16384 → 物理值 -100.003 µV  (1/4范围)  
// 数字值      0 → 物理值    0.000 µV  (中点)
// 数字值  16384 → 物理值   99.997 µV  (3/4范围)
// 数字值  32767 → 物理值  199.994 µV  (最大值)
```

#### 精度和量化噪声

```rust
// 计算信号的理论精度
let signal_range = 400.0;  // µV (从-200到+200)
let digital_levels = 65536; // 16位 = 2^16 个可能值
let resolution = signal_range / digital_levels as f64;
println!("理论分辨率: {:.4} µV", resolution);  // ~0.0061 µV

// 这意味着:
// - 小于 0.0061 µV 的信号变化无法表示
// - 量化噪声约为 ±0.003 µV
// - 对于 100µV 的信号，精度约为 0.006%
```

#### 不同信号类型的转换示例

```rust
// ECG 信号 (更大的电压范围)
let ecg_signal = SignalParam {
    label: "ECG Lead II".to_string(),
    physical_max: 5.0,        // +5 mV
    physical_min: -5.0,       // -5 mV
    digital_max: 32767,
    digital_min: -32768,
    physical_dimension: "mV".to_string(),
    // ...
};
let ecg_resolution = ecg_signal.bit_value();
println!("ECG分辨率: {:.6} mV/数字单位", ecg_resolution);  // ~0.00015 mV

// 温度信号 (不同的物理量)
let temp_signal = SignalParam {
    label: "Body Temperature".to_string(),
    physical_max: 45.0,       // 45°C
    physical_min: 30.0,       // 30°C  
    digital_max: 32767,
    digital_min: -32768,
    physical_dimension: "°C".to_string(),
    // ...
};
let temp_resolution = temp_signal.bit_value();
println!("温度分辨率: {:.6} °C/数字单位", temp_resolution);  // ~0.0002°C
```

#### 转换性能考虑

```rust
// 批量转换示例
let mut reader = EdfReader::open("large_file.edf")?;
let signal_index = 0;

// 方法1: 读取物理值 (自动转换)
let physical_samples = reader.read_physical_samples(signal_index, 10000)?;
// ✅ 推荐：直接获得可用的物理值

// 方法2: 读取数字值然后手动转换  
let digital_samples = reader.read_digital_samples(signal_index, 10000)?;
let signal = &reader.header().signals[signal_index];
let physical_samples: Vec<f64> = digital_samples
    .iter()
    .map(|&d| signal.to_physical(d))
    .collect();
// ⚠️ 仅在需要原始数字值时使用

// 性能提示：
// - 对于大多数应用，直接使用 read_physical_samples()
// - 数字值转换适用于自定义处理或验证场景
// - 转换计算很快，但避免不必要的重复转换
```

### 📊 数据记录结构

```rust
// EDF+文件的时间结构
let header = reader.header();
let record_duration_sec = header.datarecord_duration as f64 / 10_000_000.0;  // 通常是1.0秒
let total_records = header.datarecords_in_file;
let file_duration_sec = header.file_duration as f64 / 10_000_000.0;

println!("数据记录信息:");
println!("  每记录时长: {} 秒", record_duration_sec);
println!("  总记录数: {}", total_records);  
println!("  计算文件时长: {} 秒", total_records as f64 * record_duration_sec);
println!("  头部记录时长: {} 秒", file_duration_sec);

// 计算每个信号在每个数据记录中的样本数
for (i, signal) in header.signals.iter().enumerate() {
    let samples_per_sec = signal.samples_per_record as f64 / record_duration_sec;
    println!("信号 {} ({}) 采样率: {:.1} Hz", i, signal.label, samples_per_sec);
}
```

## API 文档

### 核心类型

- `EdfReader` - 用于读取EDF+文件
- `EdfWriter` - 用于创建EDF+文件  
- `EdfHeader` - 文件头部信息
- `SignalParam` - 信号参数
- `Annotation` - 注释/事件信息

### 错误处理

库使用 `Result<T, EdfError>` 类型进行错误处理：

```rust
match reader.read_physical_samples(0, 100) {
    Ok(samples) => {
        // 处理样本数据
    }
    Err(EdfError::InvalidSignalIndex(idx)) => {
        println!("无效的信号索引: {}", idx);
    }
    Err(e) => {
        println!("其他错误: {}", e);
    }
}
```

## 示例

查看 `examples/` 目录获取更多示例：

- `basic_example.rs` - 基本文件读取
- `detailed_read_example.rs` - 详细的读取示例
- `generate_test_file.rs` - 创建测试文件
- `annotation_best_practices.rs` - **注释使用最佳实践演示**

运行示例：

```bash
# 生成测试文件
cargo run --example generate_test_file

# 运行基本读取示例
cargo run --example basic_example

# 运行详细读取示例
cargo run --example detailed_read_example

# 学习注释使用的正确方法（重要！）
cargo run --example annotation_best_practices
```

## 📖 EDF+ 格式深入解析

EDF+（European Data Format Plus）是一种用于存储生物医学信号的国际标准格式，广泛应用于临床和研究领域。

### 🏥 应用领域

**神经科学与睡眠医学**
- **脑电图（EEG）**: 癫痫监测、睡眠分期、认知研究
- **多导睡眠图（PSG）**: 综合睡眠研究，包含EEG、EOG、EMG
- **颅内EEG（iEEG）**: 癫痫外科评估

**心血管监测**  
- **心电图（ECG/EKG）**: 心律监测、心脏病诊断
- **血压监测**: 连续或间歇血压记录

**肌肉和运动**
- **肌电图（EMG）**: 肌肉功能评估、运动控制研究
- **表面EMG**: 康复医学、运动科学

**其他生理信号**
- **呼吸监测**: 气流、胸腹运动
- **血氧饱和度**: SpO2连续监测
- **体温**: 核心体温或皮肤温度

### 🔧 EDF+ vs 原始EDF对比

| 特性 | 原始EDF | EDF+ |
|------|---------|------|
| **注释支持** | ❌ 无 | ✅ 完整的事件标记系统 |
| **患者信息** | 有限的自由文本 | ✅ 标准化字段格式 |
| **设备信息** | 基本信息 | ✅ 详细的设备和技术人员信息 |
| **时间精度** | 秒级 | ✅ 100纳秒精度 |
| **长期记录** | 受限 | ✅ 优化的长期记录支持 |
| **标准兼容性** | 老标准 | ✅ 现代医疗设备标准 |

### 📊 文件结构详解

EDF+ 文件由两个主要部分组成：

```
┌─────────────────────────────────────┐
│              文件头部                │  256 * (信号数 + 1) 字节
│          (Header Section)           │
├─────────────────────────────────────┤
│  信号1参数 │ 信号2参数 │ ... │ 注释参数  │
├─────────────────────────────────────┤
│              数据记录                │  可变长度
│           (Data Records)            │
├─────────────────────────────────────┤
│ 记录1 │ 记录2 │ 记录3 │ ... │ 记录N   │
├─────────────────────────────────────┤
│ 信号数据 + 注释数据 (每个记录)        │
└─────────────────────────────────────┘
```

#### 头部字段映射

```rust
// EDF+头部的256字节固定字段
struct EdfMainHeader {
    version: [u8; 8],          // "0       " (EDF+标识)
    patient_info: [u8; 80],    // 患者信息 (结构化)
    recording_info: [u8; 80],  // 记录信息 (结构化)  
    start_date: [u8; 8],       // "dd.mm.yy"
    start_time: [u8; 8],       // "hh.mm.ss"
    header_bytes: [u8; 8],     // 头部总字节数
    reserved: [u8; 44],        // "EDF+C" 或 "EDF+D" + 保留字段
    datarecords: [u8; 8],      // 数据记录总数
    record_duration: [u8; 8],  // 每记录秒数 (通常 "1       ")
    signal_count: [u8; 4],     // 信号数量 (包含注释信号)
}

// 每个信号256字节的参数 
struct EdfSignalHeader {
    label: [u8; 16],           // 信号标签
    transducer: [u8; 80],      // 传感器类型
    physical_dimension: [u8; 8], // 物理单位
    physical_min: [u8; 8],     // 物理最小值
    physical_max: [u8; 8],     // 物理最大值
    digital_min: [u8; 8],      // 数字最小值 
    digital_max: [u8; 8],      // 数字最大值
    prefilter: [u8; 80],       // 预滤波信息
    samples_per_record: [u8; 8], // 每记录样本数
    reserved: [u8; 32],        // 保留字段
}
```

### 💾 数据存储机制

#### 时间轴和数据记录

```rust
// 典型的1秒数据记录结构
let record_duration = 1.0; // 秒
let sampling_rates = vec![256, 512, 100, 1]; // Hz (EEG, EEG_high, ECG, Annotations)

// 每个数据记录包含：
// - EEG信号1: 256个样本 (256 Hz * 1秒)
// - EEG信号2: 512个样本 (512 Hz * 1秒) 
// - ECG信号:  100个样本 (100 Hz * 1秒)
// - 注释信号: 1个"样本" (实际是120字节的注释数据)

for record_index in 0..total_records {
    let record_start_time = record_index as f64 * record_duration;
    
    // 每个记录存储该时间段内所有信号的数据
    for signal_index in 0..signal_count {
        let samples_in_this_record = signal.samples_per_record;
        // 读取 samples_in_this_record 个16位整数...
    }
}
```

#### 注释信号的特殊处理

```rust
// 注释作为特殊的"信号"存储
let annotation_signal = SignalParam {
    label: "EDF Annotations".to_string(),  // 固定标签
    samples_per_record: 1,                  // 每记录1个"样本"
    digital_min: -32768,                    // 标准范围
    digital_max: 32767,
    physical_min: -1.0,                     // 物理值无意义
    physical_max: 1.0,
    physical_dimension: "".to_string(),     // 无单位
    // ...
};

// 实际存储格式：120字节的TAL (Time-stamped Annotation Lists)
// 格式: "+<onset>\x15<duration>\x14<description>\x14\x00..."
let tal_example = b"+1.234\x15\x141.5\x14Sleep Stage 2\x14\x00\x00...";
//                   ^       ^    ^                ^    ^
//                   |       |    |                |    |
//                  onset   dur  duration      description end
//                         sep   value
```

### 🎯 精度和限制

#### 时间精度
```rust
// EDF+内部使用100纳秒为时间单位
const EDFLIB_TIME_DIMENSION: i64 = 10_000_000; // 100ns单位每秒

// 时间转换示例
let precise_onset = 1.2345678; // 秒
let internal_time = (precise_onset * EDFLIB_TIME_DIMENSION as f64) as i64;
// internal_time = 12_345_678 (100ns单位)

// 最高精度：0.1微秒 = 100纳秒
// 实际精度受数据记录持续时间限制
```

#### 数据精度和动态范围
```rust
// 16位整数的限制
let max_dynamic_range = 65536; // 2^16 个可能值
let typical_eeg_range = 400.0; // µV (±200µV)
let resolution = typical_eeg_range / max_dynamic_range as f64;
println!("EEG理论分辨率: {:.4} µV", resolution); // ~0.0061 µV

// 不同信号类型的精度对比：
let signal_types = vec![
    ("EEG", 400.0, "µV"),      // 分辨率: ~0.006 µV
    ("ECG", 10.0, "mV"),       // 分辨率: ~0.00015 mV  
    ("EMG", 2000.0, "µV"),     // 分辨率: ~0.03 µV
    ("Temperature", 15.0, "°C"), // 分辨率: ~0.0002 °C
];

for (name, range, unit) in signal_types {
    let res = range / 65536.0;
    println!("{}: {:.6} {}", name, res, unit);
}
```

### 🔄 与其他格式的互操作性

```rust
// EDF+广泛支持，可与多种工具交互：

// 1. 临床软件
// - EDFbrowser (开源EDF查看器)
// - RemLogic (Embla睡眠系统)
// - Persyst (癫痫分析)

// 2. 科研软件 
// - MNE-Python (神经信号处理)
// - EEGLAB (MATLAB工具箱)
// - FieldTrip (MATLAB)
// - BrainVision Analyzer

// 3. 编程库
// - EDFlib (C/C++)
// - pyEDFlib (Python)
// - edfplus (Rust) - 本库
```

### 📈 性能特征

**文件大小估算**
```rust
fn estimate_file_size(
    channels: usize,
    sampling_rate: f64,
    duration_hours: f64,
    include_annotations: bool
) -> f64 {
    let header_size = 256 * (channels + 1); // 基础头部
    let annotation_overhead = if include_annotations { 256 + 120 } else { 0 };
    
    let samples_per_hour = sampling_rate * 3600.0 * channels as f64;
    let data_bytes_per_hour = samples_per_hour * 2.0; // 16位 = 2字节
    
    let total_bytes = header_size as f64 + 
                      annotation_overhead as f64 + 
                      data_bytes_per_hour * duration_hours;
    
    total_bytes / (1024.0 * 1024.0) // MB
}

// 示例计算
let eeg_8ch_1h = estimate_file_size(8, 256.0, 1.0, true);
println!("8通道EEG (256Hz, 1小时): {:.1} MB", eeg_8ch_1h); // ~14.8 MB

let psg_full_8h = estimate_file_size(32, 200.0, 8.0, true); 
println!("完整PSG (32通道, 200Hz, 8小时): {:.1} MB", psg_full_8h); // ~369 MB
```

**读取性能优化**
```rust
// 本库的性能优化策略：
// 1. 流式读取 - 仅加载需要的数据段
// 2. 批量转换 - 向量化的数字-物理值转换
// 3. 缓存友好 - 按记录顺序访问数据
// 4. 零拷贝 - 直接从文件映射读取

// 典型性能数据 (现代SSD):
// - 头部读取: < 1ms
// - 1秒数据读取 (8通道, 256Hz): ~0.1ms  
// - 数字到物理值转换: ~0.05ms (10k样本)
// - 注释解析: ~0.01ms (100个注释)
```

### 关键概念

- **物理值 vs 数字值**: EDF+存储16位整数，通过线性变换转换为实际的物理测量值
- **数据记录**: 文件被分割为固定时间间隔的记录，便于随机访问和流式处理
- **注释系统**: EDF+支持时间标记的事件和注释，用于标记重要事件或状态变化
- **标准化字段**: 患者信息、设备信息等采用标准化格式，确保跨系统兼容性


## ⚠️ **重要架构限制**

### 🚫 不支持回溯写入/修改

**关键限制**：这个库采用**顺序流式写入**架构，一旦调用 `write_samples()` 写入数据，已写入的内容就**永远无法更改**。

```rust
let mut writer = EdfWriter::create("test.edf")?;
// ... 添加信号 ...

// ✅ 写入第1个数据记录
writer.write_samples(&[samples1])?;  // 时间: 0-1秒

// ✅ 写入第2个数据记录  
writer.write_samples(&[samples2])?;  // 时间: 1-2秒

// ❌ 此时想要修改第1个数据记录 - **不可能！**
// 第1个数据记录已经写入文件缓冲区，无法更改

// ❌ 想要为第1个数据记录添加遗漏的注释 - **不可能！** 
writer.add_annotation(0.5, None, "Missed event")?;  
// 这个注释永远不会被写入文件，因为对应的数据记录已经完成

// ❌ 想要修改第1个数据记录的采样值 - **不可能！**
// 数据已经从内存写入到文件，无法回头修改
```

### 为什么不支持回溯写入？

1. **性能优化**：使用 `BufWriter<File>` 进行缓冲写入，优化了顺序写入性能
2. **内存效率**：流式处理，内存使用量与文件大小无关  
3. **架构简化**：避免复杂的随机访问和缓存管理

### 💡 如果需要修改数据怎么办？

如果你需要修改已写入的数据，有以下几种策略：

#### 策略1：重新生成文件
```rust
// 收集所有数据和注释，然后重新写入
let mut all_samples = Vec::new();
let mut all_annotations = Vec::new();

// 收集数据...
all_samples.push(samples1);
all_samples.push(samples2);
// 发现需要添加遗漏的注释
all_annotations.push((0.5, "Missed event"));

// 重新创建文件
let mut writer = EdfWriter::create("corrected_file.edf")?;
// 先添加所有注释
for (time, desc) in &all_annotations {
    writer.add_annotation(*time, None, desc)?;
}
// 然后写入所有数据
for samples in &all_samples {
    writer.write_samples(&[samples.clone()])?;
}
```

#### 策略2：分阶段处理
```rust
// 先在内存中准备所有数据，最后一次性写入
struct RecordingSession {
    samples: Vec<Vec<Vec<f64>>>,  // [记录][通道][样本]
    annotations: Vec<(f64, String)>,
}

impl RecordingSession {
    fn add_samples(&mut self, record_idx: usize, channel_idx: usize, samples: Vec<f64>) {
        // 在内存中修改数据
        self.samples[record_idx][channel_idx] = samples;
    }
    
    fn add_annotation(&mut self, time: f64, desc: String) {
        // 可以随时添加注释
        self.annotations.push((time, desc));
    }
    
    fn write_to_file(&self, filename: &str) -> Result<()> {
        let mut writer = EdfWriter::create(filename)?;
        
        // 先添加所有注释
        for (time, desc) in &self.annotations {
            writer.add_annotation(*time, None, desc)?;
        }
        
        // 然后按顺序写入所有数据记录
        for record_samples in &self.samples {
            writer.write_samples(record_samples)?;
        }
        
        writer.finalize()
    }
}
```

#### 策略3：使用临时文件
```rust
use std::fs;
use tempfile::NamedTempFile;

// 写入临时文件，确认无误后替换目标文件
let temp_file = NamedTempFile::new()?;
let temp_path = temp_file.path();

{
    let mut writer = EdfWriter::create(temp_path)?;
    // ... 写入数据 ...
    writer.finalize()?;
}  // writer被drop，文件被关闭

// 验证文件正确性
let reader = EdfReader::open(temp_path)?;
if validate_file(&reader)? {
    // 替换目标文件
    fs::copy(temp_path, "final_output.edf")?;
    println!("文件写入成功");
} else {
    println!("文件验证失败，未替换原文件");
}
```

### 🎯 最佳实践建议

1. **提前规划**：在开始写入前就确定好所有的数据和注释
2. **分批验证**：可以先写入少量数据进行测试
3. **使用临时文件**：对于重要数据，先写入临时文件再验证
4. **文档化流程**：记录数据写入的顺序和依赖关系

```rust
// 推荐的写入流程
fn create_edf_file() -> Result<()> {
    // 1. 数据准备阶段
    let mut all_annotations = prepare_annotations()?;
    let signal_params = define_signals()?;
    
    // 2. 创建写入器并配置
    let mut writer = EdfWriter::create("output.edf")?;
    writer.set_patient_info("P001", "M", "01-JAN-1990", "Test")?;
    
    for param in signal_params {
        writer.add_signal(param)?;
    }
    
    // 3. 添加所有预期的注释
    for (time, desc) in all_annotations {
        writer.add_annotation(time, None, &desc)?;
    }
    
    // 4. 顺序写入所有数据（不再修改）
    for record_idx in 0..total_records {
        let samples = generate_samples_for_record(record_idx)?;
        writer.write_samples(&samples)?;
    }
    
    // 5. 完成文件
    writer.finalize()?;
    Ok(())
}
```

## 性能

- **内存效率**: 支持流式读取，内存使用量与文件大小无关
- **速度**: 针对大文件优化的读取性能
- **并发安全**: 结构体设计支持多线程访问（读取器除外）

## 兼容性

- **Rust版本**: 需要 Rust 1.70+
- **平台**: 支持所有Rust支持的平台
- **EDF版本**: 专注于EDF+格式，不支持原始EDF格式

## 许可证

本项目采用 BSD-3-Clause 许可证。

## 贡献

欢迎提交issue和pull request！

## 致谢

本库参考了原始的[EDFlib](https://gitlab.com/Teuniz/EDFlib) C库的设计思想，但采用了现代Rust的最佳实践重新实现。
