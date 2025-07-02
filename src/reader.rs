use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use chrono::{NaiveDate, NaiveTime};

use crate::types::{EdfHeader, SignalParam, Annotation};
use crate::error::{EdfError, Result};
use crate::utils::{atoi_nonlocalized, atof_nonlocalized, parse_edf_time};
use crate::EDFLIB_TIME_DIMENSION;

/// TAL parsing state machine states
#[derive(Debug, Clone, PartialEq)]
enum TalState {
    WaitingForOnset,        // Waiting for onset field (start of TAL)
    CollectingOnset,        // Collecting onset time characters
    CollectingDuration,     // Collecting duration characters
    CollectingDescription,  // Collecting description text
}

/// EDF+ file reader for reading European Data Format Plus files
/// 
/// The `EdfReader` provides methods to open and read EDF+ files, which are
/// commonly used for storing biosignal recordings like EEG, ECG, EMG, etc.
/// 
/// # Examples
/// 
/// ## Basic usage
/// 
/// ```rust
/// use edfplus::EdfReader;
/// 
/// # // Generate test file (hidden from docs)
/// # edfplus::doctest_utils::create_simple_test_file("recording.edf")?;
/// # 
/// // Open an EDF+ file
/// let mut reader = EdfReader::open("recording.edf")?;
/// 
/// // Get header information
/// let header = reader.header();
/// println!("Duration: {:.1} seconds", header.file_duration as f64 / 10_000_000.0);
/// println!("Signals: {}", header.signals.len());
/// 
/// // Read physical samples from first signal
/// let samples = reader.read_physical_samples(0, 256)?;
/// println!("Read {} samples", samples.len());
/// 
/// # // Cleanup (hidden from docs)
/// # std::fs::remove_file("recording.edf").ok();
/// # Ok::<(), edfplus::EdfError>(())
/// ```
/// ## Processing all signals
/// 
/// ```rust
/// use edfplus::EdfReader;
/// 
/// # // Generate test file (hidden from docs)
/// # edfplus::doctest_utils::create_multi_channel_test_file("multi_signal.edf")?;
/// # 
/// let mut reader = EdfReader::open("multi_signal.edf")?;
/// let signal_count = reader.header().signals.len();
/// 
/// // Process each signal
/// for i in 0..signal_count {
///     let signal_label = reader.header().signals[i].label.clone();
///     let signal_dimension = reader.header().signals[i].physical_dimension.clone();
///     let samples_per_second = reader.header().signals[i].samples_per_record as usize;
///     
///     println!("Processing signal {}: {}", i, signal_label);
///     
///     // Read one second of data (assuming 256 Hz sampling rate)
///     let physical_values = reader.read_physical_samples(i, samples_per_second)?;
///     
///     // Calculate basic statistics
///     let mean = physical_values.iter().sum::<f64>() / physical_values.len() as f64;
///     let max = physical_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
///     let min = physical_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
///     
///     println!("  Mean: {:.2} {}", mean, signal_dimension);
///     println!("  Range: {:.2} to {:.2} {}", min, max, signal_dimension);
/// }
/// 
/// # // Cleanup (hidden from docs)
/// # std::fs::remove_file("multi_signal.edf").ok();
/// # Ok::<(), edfplus::EdfError>(())
/// ```
pub struct EdfReader {
    file: BufReader<File>,
    header: EdfHeader,
    /// 每个信号在文件中的位置信息
    signal_info: Vec<SignalInfo>,
    /// 当前每个信号的样本位置指针
    sample_positions: Vec<i64>,
    /// 文件的头部大小
    header_size: usize,
    /// 每个数据记录的大小（字节）
    record_size: usize,
    /// 注释列表
    annotations: Vec<Annotation>,
}

#[derive(Debug, Clone)]
struct SignalInfo {
    /// 信号在数据记录中的字节偏移
    buffer_offset: usize,
    /// 每个数据记录中的样本数
    samples_per_record: i32,
    /// 是否是注释信号
    is_annotation: bool,
}

impl EdfReader {
    /// Opens an EDF+ file for reading
    /// 
    /// This method opens the specified file, validates it as a proper EDF+ file,
    /// and parses the header information. Only EDF+ format is supported.
    /// 
    /// # Arguments
    /// 
    /// * `path` - Path to the EDF+ file to open
    /// 
    /// # Returns
    /// 
    /// Returns a `Result<EdfReader, EdfError>`. On success, contains an `EdfReader`
    /// instance ready for reading data. On failure, contains an error describing
    /// what went wrong.
    /// 
    /// # Errors
    /// 
    /// * `EdfError::FileNotFound` - File doesn't exist or can't be opened
    /// * `EdfError::UnsupportedFileType` - File is not EDF+ format
    /// * `EdfError::InvalidHeader` - File header is corrupted or invalid
    /// * `EdfError::InvalidSignalCount` - Invalid number of signals
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::EdfReader;
    /// 
    /// # // Generate test file (hidden from docs)
    /// # edfplus::doctest_utils::create_simple_test_file("recording.edf")?;
    /// # 
    /// // Open a file successfully
    /// match EdfReader::open("recording.edf") {
    ///     Ok(reader) => {
    ///         println!("File opened successfully!");
    ///         println!("Duration: {:.1} seconds", 
    ///             reader.header().file_duration as f64 / 10_000_000.0);
    ///     }
    ///     Err(e) => eprintln!("Failed to open file: {}", e),
    /// }
    /// 
    /// // Handle different error types
    /// match EdfReader::open("nonexistent.edf") {
    ///     Ok(_) => println!("Unexpected success"),
    ///     Err(edfplus::EdfError::FileNotFound(msg)) => {
    ///         println!("File not found: {}", msg);
    ///     }
    ///     Err(e) => println!("Other error: {}", e),
    /// }
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("recording.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(&path)
            .map_err(|e| EdfError::FileNotFound(format!("{}: {}", path.as_ref().display(), e)))?;
        
        let mut reader = BufReader::new(file);
        
        // 读取并解析头部
        let (mut header, signal_info, record_size) = Self::parse_header(&mut reader)?;
        
        // 计算头部大小
        let total_signals = signal_info.len();
        let header_size = (total_signals + 1) * 256;
        
        // 初始化样本位置指针
        let sample_positions = vec![0i64; header.signals.len()];
        
        // 解析注释以获取准确的注释数量和可能的subsecond时间
        let (annotations_count, starttime_subsecond) = Self::count_annotations_and_parse_subsecond(
            &mut reader, 
            &signal_info, 
            header.datarecords_in_file,
            record_size,
            header_size
        ).unwrap_or((0, 0));
        
        // 更新头部信息
        header.annotations_in_file = annotations_count;
        header.starttime_subsecond = starttime_subsecond;
        
        // 创建读取器实例
        let mut temp_reader = EdfReader {
            file: reader,
            header,
            signal_info,
            sample_positions,
            header_size,
            record_size,
            annotations: Vec::new(),
        };
        
        // 解析注释数据
        let annotations = temp_reader.parse_annotations().unwrap_or_else(|_| Vec::new());
        temp_reader.annotations = annotations;
        
        Ok(temp_reader)
    }
    
    /// Gets a reference to the file header information
    /// 
    /// The header contains all metadata about the recording including:
    /// - Patient information (name, code, birth date, etc.)
    /// - Recording information (start time, duration, equipment, etc.)
    /// - Signal parameters (labels, sampling rates, physical ranges, etc.)
    /// - File format details
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::EdfReader;
    /// 
    /// # // Generate test file (hidden from docs) 
    /// # edfplus::doctest_utils::create_simple_test_file("recording.edf")?;
    /// # 
    /// let reader = EdfReader::open("recording.edf")?;
    /// let header = reader.header();
    /// 
    /// // Display basic file information
    /// println!("Patient: {}", header.patient_name);
    /// println!("Recording duration: {:.2} seconds", 
    ///     header.file_duration as f64 / 10_000_000.0);
    /// println!("Number of signals: {}", header.signals.len());
    /// 
    /// // Display signal information
    /// for (i, signal) in header.signals.iter().enumerate() {
    ///     println!("Signal {}: {} ({})", 
    ///         i, signal.label, signal.physical_dimension);
    ///     println!("  Sample rate: {} Hz", signal.samples_per_record);
    ///     println!("  Range: {} to {} {}", 
    ///         signal.physical_min, signal.physical_max, signal.physical_dimension);
    /// }
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("recording.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    pub fn header(&self) -> &EdfHeader {
        &self.header
    }
    
    /// Gets a reference to the list of annotations in the file
    /// 
    /// Annotations represent events, markers, and metadata that occurred
    /// during the recording. Common examples include sleep stages, seizures,
    /// artifacts, stimuli, and user-defined events.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::{EdfReader, EdfWriter, SignalParam, Annotation};
    /// # use std::fs;
    /// 
    /// # // Create a test file (without annotations for this example)
    /// # let mut writer = EdfWriter::create("test_annotations.edf").unwrap();
    /// # writer.set_patient_info("P001", "M", "01-JAN-1990", "Test Patient").unwrap();
    /// # let signal = SignalParam {
    /// #     label: "EEG".to_string(),
    /// #     samples_in_file: 0,
    /// #     physical_max: 100.0,
    /// #     physical_min: -100.0,
    /// #     digital_max: 32767,
    /// #     digital_min: -32768,
    /// #     samples_per_record: 256,
    /// #     physical_dimension: "uV".to_string(),
    /// #     prefilter: "HP:0.1Hz".to_string(),
    /// #     transducer: "AgAgCl".to_string(),
    /// # };
    /// # writer.add_signal(signal).unwrap();
    /// # let samples = vec![10.0; 256];
    /// # writer.write_samples(&[samples]).unwrap();
    /// # writer.finalize().unwrap();
    /// 
    /// let reader = EdfReader::open("test_annotations.edf").unwrap();
    /// let annotations = reader.annotations();
    /// 
    /// println!("Found {} annotations", annotations.len());
    /// 
    /// for (i, annotation) in annotations.iter().enumerate() {
    ///     let onset_seconds = annotation.onset as f64 / 10_000_000.0;
    ///     let duration_seconds = if annotation.duration >= 0 {
    ///         annotation.duration as f64 / 10_000_000.0
    ///     } else {
    ///         0.0  // Instantaneous event
    ///     };
    ///     
    ///     println!("Annotation {}: {} at {:.2}s (duration: {:.2}s)",
    ///         i, annotation.description, onset_seconds, duration_seconds);
    /// }
    /// 
    /// # // Cleanup
    /// # drop(reader);
    /// # fs::remove_file("test_annotations.edf").ok();
    /// ```
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }
    
    /// Reads physical value samples from the specified signal
    /// 
    /// Physical values are the real-world measurements (e.g., microvolts for EEG,
    /// millivolts for ECG) as opposed to the raw digital values stored in the file.
    /// The conversion from digital to physical values is performed automatically
    /// using the signal's calibration parameters.
    /// 
    /// # Arguments
    /// 
    /// * `signal` - Zero-based index of the signal to read from
    /// * `count` - Number of samples to read
    /// 
    /// # Returns
    /// 
    /// Vector of physical values in the signal's physical dimension (e.g., µV, mV).
    /// 
    /// # Errors
    /// 
    /// * `EdfError::InvalidSignalIndex` - Signal index is out of bounds
    /// * `EdfError::FileReadError` - I/O error reading from file
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::EdfReader;
    /// 
    /// # // Generate test file (hidden from docs)
    /// # edfplus::doctest_utils::create_simple_test_file("eeg_recording.edf")?;
    /// # 
    /// let mut reader = EdfReader::open("eeg_recording.edf")?;
    /// 
    /// // Read 1 second of EEG data (assuming 256 Hz)
    /// let samples = reader.read_physical_samples(0, 256)?;
    /// 
    /// // Get header after reading samples
    /// let header = reader.header();
    /// 
    /// // Calculate basic statistics
    /// let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    /// let max_value = samples.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    /// let min_value = samples.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    /// 
    /// println!("Signal: {}", header.signals[0].label);
    /// println!("Mean: {:.2} {}", mean, header.signals[0].physical_dimension);
    /// println!("Range: {:.2} to {:.2} {}", 
    ///     min_value, max_value, header.signals[0].physical_dimension);
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("eeg_recording.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    /// 
    /// ## Processing multiple signals
    /// 
    /// ```rust
    /// use edfplus::EdfReader;
    /// 
    /// # // Generate test file (hidden from docs)
    /// # edfplus::doctest_utils::create_multi_channel_test_file("multi_channel.edf")?;
    /// # 
    /// let mut reader = EdfReader::open("multi_channel.edf")?;
    /// let signal_count = reader.header().signals.len();
    /// 
    /// // Read data from all signals  
    /// for signal_idx in 0..signal_count {
    ///     let signal_label = reader.header().signals[signal_idx].label.clone();
    ///     let signal_dimension = reader.header().signals[signal_idx].physical_dimension.clone();
    ///     let samples_per_record = reader.header().signals[signal_idx].samples_per_record as usize;
    ///     
    ///     // Read one record worth of data (safe amount)
    ///     let samples = reader.read_physical_samples(signal_idx, samples_per_record)?;
    ///     
    ///     println!("Signal {}: {} samples from {}", 
    ///         signal_label, samples.len(), signal_dimension);
    ///         
    ///     // Find peak-to-peak amplitude
    ///     let max = samples.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    ///     let min = samples.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    ///     println!("  Amplitude: {:.2} {}", max - min, signal_dimension);
    /// }
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("multi_channel.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    pub fn read_physical_samples(&mut self, signal: usize, count: usize) -> Result<Vec<f64>> {
        let digital_samples = self.read_digital_samples(signal, count)?;
        
        if signal >= self.header.signals.len() {
            return Err(EdfError::InvalidSignalIndex(signal));
        }
        
        let signal_param = &self.header.signals[signal];
        let physical_samples = digital_samples
            .into_iter()
            .map(|d| signal_param.to_physical(d))
            .collect();
        
        Ok(physical_samples)
    }
    
    /// Reads digital value samples from the specified signal
    /// 
    /// Digital values are the raw integer values stored in the EDF+ file,
    /// before conversion to physical units. These are typically 16-bit
    /// signed integers representing the ADC output.
    /// 
    /// Most users should use `read_physical_samples()` instead, which
    /// automatically converts to real-world units.
    /// 
    /// # Arguments
    /// 
    /// * `signal` - Zero-based index of the signal to read from  
    /// * `count` - Number of samples to read
    /// 
    /// # Returns
    /// 
    /// Vector of digital values as signed 32-bit integers.
    /// 
    /// # Errors
    /// 
    /// * `EdfError::InvalidSignalIndex` - Signal index is out of bounds
    /// * `EdfError::FileReadError` - I/O error reading from file
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::EdfReader;
    /// 
    /// # // Generate test file (hidden from docs)
    /// # edfplus::doctest_utils::create_simple_test_file("recording.edf")?;
    /// # 
    /// let mut reader = EdfReader::open("recording.edf")?;
    /// 
    /// // Read raw digital values
    /// let digital_samples = reader.read_digital_samples(0, 100)?;
    /// 
    /// // Get header after reading
    /// let header = reader.header();
    /// let signal = &header.signals[0];
    /// 
    /// // Manual conversion to physical values
    /// let physical_samples: Vec<f64> = digital_samples
    ///     .iter()
    ///     .map(|&d| signal.to_physical(d))
    ///     .collect();
    /// 
    /// println!("Digital range: {} to {}", 
    ///     digital_samples.iter().min().unwrap(),
    ///     digital_samples.iter().max().unwrap());
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("recording.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    /// 
    /// ## Checking digital value ranges
    /// 
    /// ```rust
    /// use edfplus::EdfReader;
    /// 
    /// # // Generate test file (hidden from docs)
    /// # edfplus::doctest_utils::create_validation_test_file("test.edf")?;
    /// # 
    /// let mut reader = EdfReader::open("test.edf")?;
    /// let signal_count = reader.header().signals.len();
    /// 
    /// for i in 0..signal_count {
    ///     let signal_label = reader.header().signals[i].label.clone();
    ///     let digital_min = reader.header().signals[i].digital_min;
    ///     let digital_max = reader.header().signals[i].digital_max;
    ///     
    ///     let samples = reader.read_digital_samples(i, 10)?;
    ///     
    ///     let min_val = *samples.iter().min().unwrap();
    ///     let max_val = *samples.iter().max().unwrap();
    ///     
    ///     println!("Signal {}: digital range {} to {} (expected: {} to {})",
    ///         signal_label, min_val, max_val, digital_min, digital_max);
    ///         
    ///     // Check for clipping
    ///     if min_val <= digital_min || max_val >= digital_max {
    ///         println!("  Warning: Signal may be clipped!");
    ///     }
    /// }
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("test.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    pub fn read_digital_samples(&mut self, signal: usize, count: usize) -> Result<Vec<i32>> {
        if signal >= self.header.signals.len() {
            return Err(EdfError::InvalidSignalIndex(signal));
        }
        
        if count == 0 {
            return Ok(Vec::new());
        }
        
        // 找到实际的信号索引（跳过注释信号）
        let mut actual_signal_idx = 0;
        let mut user_signal_count = 0;
        
        for i in 0..self.signal_info.len() {
            if !self.signal_info[i].is_annotation {
                if user_signal_count == signal {
                    actual_signal_idx = i;
                    break;
                }
                user_signal_count += 1;
            }
        }
        
        let signal_info = &self.signal_info[actual_signal_idx];
        let signal_param = &self.header.signals[signal];
        
        // 计算可读取的最大样本数
        let samples_in_file = signal_param.samples_per_record as i64 * self.header.datarecords_in_file;
        let available_samples = (samples_in_file - self.sample_positions[signal]).max(0) as usize;
        let actual_count = count.min(available_samples);
        
        if actual_count == 0 {
            return Ok(Vec::new());
        }
        
        let mut samples = Vec::with_capacity(actual_count);
        let mut samples_read = 0;
        let current_pos = self.sample_positions[signal];
        
        // ✅ 性能优化：使用类似 edflib 的直接计算方式
        while samples_read < actual_count {
            let pos = current_pos + samples_read as i64;
            let record_index = pos / signal_param.samples_per_record as i64;
            let sample_in_record = pos % signal_param.samples_per_record as i64;
            
            // 计算连续可读取的样本数（避免跨记录）
            let samples_remaining_in_record = 
                (signal_param.samples_per_record as i64 - sample_in_record) as usize;
            let samples_to_read = (actual_count - samples_read).min(samples_remaining_in_record);
            
            // ✅ 使用预计算的 buffer_offset 直接定位
            let file_offset = self.header_size as u64 
                + record_index as u64 * self.record_size as u64
                + signal_info.buffer_offset as u64
                + sample_in_record as u64 * 2; // EDF每个样本2字节
            
            // 定位到正确位置
            self.file.seek(SeekFrom::Start(file_offset))?;
            
            // ✅ 批量读取以提高性能
            let bytes_to_read = samples_to_read * 2;
            let mut buffer = vec![0u8; bytes_to_read];
            self.file.read_exact(&mut buffer)?;
            
            // 转换字节到数字值并应用范围限制
            for chunk in buffer.chunks_exact(2) {
                let digital_value = i16::from_le_bytes([chunk[0], chunk[1]]) as i32;
                
                // ✅ 应用数字范围限制（类似 edflib 的 clamping）
                let clamped_value = digital_value
                    .max(signal_param.digital_min)
                    .min(signal_param.digital_max);
                
                samples.push(clamped_value);
                samples_read += 1;
                
                if samples_read >= actual_count {
                    break;
                }
            }
        }
        
        // 更新样本位置
        self.sample_positions[signal] = current_pos + samples_read as i64;
        
        Ok(samples)
    }
    
    /// 设置指定信号的样本位置
    pub fn seek(&mut self, signal: usize, position: i64) -> Result<i64> {
        if signal >= self.header.signals.len() {
            return Err(EdfError::InvalidSignalIndex(signal));
        }
        
        let signal_param = &self.header.signals[signal];
        let max_position = signal_param.samples_per_record as i64 * self.header.datarecords_in_file;
        
        let new_position = position.max(0).min(max_position);
        self.sample_positions[signal] = new_position;
        
        Ok(new_position)
    }
    
    /// 获取指定信号的当前样本位置
    pub fn tell(&self, signal: usize) -> Result<i64> {
        if signal >= self.header.signals.len() {
            return Err(EdfError::InvalidSignalIndex(signal));
        }
        
        Ok(self.sample_positions[signal])
    }
    
    /// 将指定信号的位置重置到开头
    pub fn rewind(&mut self, signal: usize) -> Result<()> {
        self.seek(signal, 0)?;
        Ok(())
    }
    
    /// 解析EDF+文件头部
    fn parse_header(reader: &mut BufReader<File>) -> Result<(EdfHeader, Vec<SignalInfo>, usize)> {
        // 读取主头部（256字节）
        reader.seek(SeekFrom::Start(0))?;
        let mut main_header = vec![0u8; 256];
        reader.read_exact(&mut main_header)?;
        
        // 验证EDF+标识
        let version = String::from_utf8_lossy(&main_header[0..8]);
        if !version.trim().starts_with('0') {
            return Err(EdfError::UnsupportedFileType(format!("Not an EDF file: {}", version)));
        }
        
        // 解析信号数量
        let signals_str = String::from_utf8_lossy(&main_header[252..256]);
        let total_signal_count = atoi_nonlocalized(&signals_str);
        if total_signal_count < 1 || total_signal_count > crate::EDFLIB_MAXSIGNALS as i32 {
            return Err(EdfError::InvalidSignalCount(total_signal_count));
        }
        
        // 验证头部大小
        let header_size_str = String::from_utf8_lossy(&main_header[184..192]);
        let expected_header_size = (total_signal_count + 1) * 256;
        let actual_header_size = atoi_nonlocalized(&header_size_str);
        if actual_header_size != expected_header_size {
            return Err(EdfError::InvalidHeader);
        }
        
        // 检查EDF+标识
        let reserved = String::from_utf8_lossy(&main_header[192..236]);
        let is_edfplus = reserved.starts_with("EDF+C");
        if !is_edfplus {
            return Err(EdfError::UnsupportedFileType("Only EDF+ files are supported".to_string()));
        }
        
        // 解析基本信息
        let patient_field = String::from_utf8_lossy(&main_header[8..88]).trim().to_string();
        let recording_field = String::from_utf8_lossy(&main_header[88..168]).trim().to_string();
        
        // 解析日期和时间
        let date_str = String::from_utf8_lossy(&main_header[168..176]);
        let time_str = String::from_utf8_lossy(&main_header[176..184]);
        
        let (start_date, start_time) = Self::parse_datetime(&date_str, &time_str)?;
        
        // 解析数据记录信息
        let datarecords_str = String::from_utf8_lossy(&main_header[236..244]);
        let datarecords = atoi_nonlocalized(&datarecords_str) as i64;
        
        let duration_str = String::from_utf8_lossy(&main_header[244..252]);
        let datarecord_duration = if duration_str.trim() == "1" {
            EDFLIB_TIME_DIMENSION
        } else {
            parse_edf_time(&duration_str)?
        };
        
        // 读取信号头部信息
        let signal_header_size = total_signal_count as usize * 256;
        let mut signal_header = vec![0u8; signal_header_size];
        reader.read_exact(&mut signal_header)?;
        
        // 解析信号参数
        let (signals, signal_info, total_record_size) = Self::parse_signals(
            &signal_header, 
            total_signal_count as usize,
            datarecords
        )?;
        
        // 解析EDF+字段
        let (patient_code, sex, birthdate, patient_name, patient_additional) = 
            Self::parse_edfplus_patient(&patient_field)?;
        
        let (admin_code, technician, equipment, recording_additional) = 
            Self::parse_edfplus_recording(&recording_field)?;
        
        // 创建临时头部用于注释解析
        let mut temp_header = EdfHeader {
            signals,
            file_duration: datarecord_duration * datarecords,
            start_date,
            start_time,
            starttime_subsecond: 0,
            datarecords_in_file: datarecords,
            datarecord_duration,
            annotations_in_file: 0,
            patient_code,
            sex,
            birthdate,
            patient_name,
            patient_additional,
            admin_code,
            technician,
            equipment,
            recording_additional,
        };
        
        // 解析注释以获取准确的注释数量和可能的subsecond时间
        let (annotations_count, starttime_subsecond) = Self::count_annotations_and_parse_subsecond(
            reader, 
            &signal_info, 
            datarecords,
            total_record_size,
            (total_signal_count as usize + 1) * 256
        ).unwrap_or((0, 0));
        
        // 更新头部信息
        temp_header.annotations_in_file = annotations_count;
        temp_header.starttime_subsecond = starttime_subsecond;
        
        Ok((temp_header, signal_info, total_record_size))
    }
    
    /// 解析日期时间
    fn parse_datetime(date_str: &str, time_str: &str) -> Result<(NaiveDate, NaiveTime)> {
        // 解析日期 "dd.mm.yy"
        let date_parts: Vec<&str> = date_str.split('.').collect();
        if date_parts.len() != 3 {
            return Err(EdfError::FormatError);
        }
        
        let day = atoi_nonlocalized(date_parts[0]);
        let month = atoi_nonlocalized(date_parts[1]);
        let year = {
            let yy = atoi_nonlocalized(date_parts[2]);
            if yy > 84 { 1900 + yy } else { 2000 + yy }
        };
        
        let start_date = NaiveDate::from_ymd_opt(year, month as u32, day as u32)
            .ok_or(EdfError::FormatError)?;
        
        // 解析时间 "hh.mm.ss"
        let time_parts: Vec<&str> = time_str.split('.').collect();
        if time_parts.len() != 3 {
            return Err(EdfError::FormatError);
        }
        
        let hour = atoi_nonlocalized(time_parts[0]);
        let minute = atoi_nonlocalized(time_parts[1]);
        let second = atoi_nonlocalized(time_parts[2]);
        
        let start_time = NaiveTime::from_hms_opt(hour as u32, minute as u32, second as u32)
            .ok_or(EdfError::FormatError)?;
        
        Ok((start_date, start_time))
    }
    
    /// 解析信号参数
    fn parse_signals(
        signal_header: &[u8], 
        total_signal_count: usize,
        datarecords: i64
    ) -> Result<(Vec<SignalParam>, Vec<SignalInfo>, usize)> {
        let mut signals = Vec::new();
        let mut signal_info = Vec::new();
        let mut buffer_offset = 0;
        
        // 解析每个信号的各个字段
        for i in 0..total_signal_count {
            // 标签 (16字节)
            let label_start = i * 16;
            let label_bytes = &signal_header[label_start..label_start + 16];
            let label = String::from_utf8_lossy(label_bytes).trim().to_string();
            
            // 检查是否是注释信号 - 必须完全匹配 "EDF Annotations " (注意末尾的空格)
            let is_annotation = label_bytes == b"EDF Annotations ";
            
            // 传感器类型 (80字节，从偏移16*signal_count开始)
            let transducer_start = total_signal_count * 16 + i * 80;
            let transducer = String::from_utf8_lossy(
                &signal_header[transducer_start..transducer_start + 80]
            ).trim().to_string();
            
            // 物理单位 (8字节)
            let unit_start = total_signal_count * 96 + i * 8;
            let physical_dimension = String::from_utf8_lossy(
                &signal_header[unit_start..unit_start + 8]
            ).trim().to_string();
            
            // 物理最小值 (8字节)
            let phys_min_start = total_signal_count * 104 + i * 8;
            let phys_min_str = String::from_utf8_lossy(
                &signal_header[phys_min_start..phys_min_start + 8]
            );
            let physical_min = atof_nonlocalized(&phys_min_str);
            
            // 物理最大值 (8字节)
            let phys_max_start = total_signal_count * 112 + i * 8;
            let phys_max_str = String::from_utf8_lossy(
                &signal_header[phys_max_start..phys_max_start + 8]
            );
            let physical_max = atof_nonlocalized(&phys_max_str);
            
            // 数字最小值 (8字节)
            let dig_min_start = total_signal_count * 120 + i * 8;
            let dig_min_str = String::from_utf8_lossy(
                &signal_header[dig_min_start..dig_min_start + 8]
            );
            let digital_min = atoi_nonlocalized(&dig_min_str);
            
            // 数字最大值 (8字节)  
            let dig_max_start = total_signal_count * 128 + i * 8;
            let dig_max_str = String::from_utf8_lossy(
                &signal_header[dig_max_start..dig_max_start + 8]
            );
            let digital_max = atoi_nonlocalized(&dig_max_str);
            
            // 预滤波 (80字节)
            let prefilter_start = total_signal_count * 136 + i * 80;
            let prefilter = String::from_utf8_lossy(
                &signal_header[prefilter_start..prefilter_start + 80]
            ).trim().to_string();
            
            // 每个数据记录中的样本数 (8字节)
            let samples_start = total_signal_count * 216 + i * 8;
            let samples_str = String::from_utf8_lossy(
                &signal_header[samples_start..samples_start + 8]
            );
            let samples_per_record = atoi_nonlocalized(&samples_str);
            
            // 创建 SignalInfo - 所有信号都要设置正确的 buffer_offset
            let info = SignalInfo {
                buffer_offset,  // 当前累计的字节偏移
                samples_per_record,
                is_annotation,
            };
            
            // 只有非注释信号才添加到用户可见的信号列表中
            if !is_annotation {
                // 验证参数
                if physical_min == physical_max {
                    return Err(EdfError::PhysicalMinEqualsMax);
                }
                if digital_min == digital_max {
                    return Err(EdfError::DigitalMinEqualsMax);
                }
                
                let signal_param = SignalParam {
                    label,
                    samples_in_file: samples_per_record as i64 * datarecords,
                    physical_max,
                    physical_min,
                    digital_max,
                    digital_min,
                    samples_per_record,
                    physical_dimension,
                    prefilter,
                    transducer,
                };
                
                signals.push(signal_param);
            }
            
            // 将信号信息添加到列表（包括注释信号）
            signal_info.push(info);
            
            // ✅ 关键修复：为所有信号（包括注释信号）更新 buffer_offset
            // 每个样本占用 2 字节（EDF 格式固定）
            buffer_offset += samples_per_record as usize * 2;
        }
        
        Ok((signals, signal_info, buffer_offset))
    }
    
    /// 解析EDF+患者字段
    fn parse_edfplus_patient(patient_field: &str) -> Result<(String, String, String, String, String)> {
        // EDF+ 患者字段格式: "patientcode sex birthdate patientname additional_info"
        let parts: Vec<&str> = patient_field.split_whitespace().collect();
        
        let patient_code = parts.get(0).unwrap_or(&"").to_string();
        let sex = parts.get(1).unwrap_or(&"").to_string();
        let birthdate = parts.get(2).unwrap_or(&"").to_string();
        let patient_name = parts.get(3).unwrap_or(&"").to_string();
        let patient_additional = parts.get(4..).map(|s| s.join(" ")).unwrap_or_default();
        
        Ok((patient_code, sex, birthdate, patient_name, patient_additional))
    }
    
    /// 解析EDF+记录字段
    fn parse_edfplus_recording(recording_field: &str) -> Result<(String, String, String, String)> {
        // EDF+ 记录字段格式: "startdate admincode technician equipment additional_info"
        let parts: Vec<&str> = recording_field.split_whitespace().collect();
        
        let admin_code = parts.get(1).unwrap_or(&"").to_string();
        let technician = parts.get(2).unwrap_or(&"").to_string();
        let equipment = parts.get(3).unwrap_or(&"").to_string();
        let recording_additional = parts.get(4..).map(|s| s.join(" ")).unwrap_or_default();
        
        Ok((admin_code, technician, equipment, recording_additional))
    }
    
    /// Parses TAL (Time-stamped Annotations Lists) data from annotation signals
    /// 
    /// This reads the annotation signal data and extracts annotations according 
    /// to the EDF+ TAL format specification, following the edflib implementation.
    fn parse_annotations(&mut self) -> Result<Vec<Annotation>> {
        let mut annotations = Vec::new();
        let mut elapsed_time = 0i64;
        
        // 找到注释信号
        let annotation_signals: Vec<usize> = self.signal_info
            .iter()
            .enumerate()
            .filter_map(|(i, info)| if info.is_annotation { Some(i) } else { None })
            .collect();
        
        if annotation_signals.is_empty() {
            return Ok(annotations);
        }
        
        // 读取每个数据记录的数据
        let datarecords = self.header.datarecords_in_file;
        let mut first_record_processed = false;
        
        for record_idx in 0..datarecords {
            // 定位到数据记录
            let record_offset = self.header_size as u64 + (record_idx as u64 * self.record_size as u64);
            self.file.seek(std::io::SeekFrom::Start(record_offset))?;
            
            // 读取整个数据记录
            let mut record_data = vec![0u8; self.record_size];
            self.file.read_exact(&mut record_data)?;
            
            // 处理每个注释信号
            for (ann_idx, &ann_signal_idx) in annotation_signals.iter().enumerate() {
                let ann_info = &self.signal_info[ann_signal_idx];
                
                // 计算注释信号在记录中的偏移
                let signal_offset = ann_info.buffer_offset;
                
                // 提取注释信号数据
                let bytes_to_read = (ann_info.samples_per_record * 2) as usize;
                if signal_offset + bytes_to_read <= record_data.len() {
                    let tal_data = &record_data[signal_offset..signal_offset + bytes_to_read];
                    
                    // 第一个注释信号需要验证时间戳
                    if ann_idx == 0 {
                        if let Some(timestamp) = self.extract_timestamp(tal_data, record_idx)? {
                            if record_idx > 0 {
                                // 验证时间连续性
                                let expected_time = elapsed_time + self.header.datarecord_duration;
                                let time_diff = (timestamp - expected_time).abs();
                                if time_diff > EDFLIB_TIME_DIMENSION / 1000 {
                                    // 时间不连续，可能是discontinuous文件
                                    return Err(EdfError::InvalidHeader);
                                }
                            } else if !first_record_processed {
                                // 第一个记录，设置subsecond偏移 (如果还没有设置)
                                if self.header.starttime_subsecond == 0 {
                                    self.header.starttime_subsecond = timestamp % EDFLIB_TIME_DIMENSION;
                                }
                                first_record_processed = true;
                            }
                            elapsed_time = timestamp;
                        }
                    }
                    
                    // 解析注释
                    let record_annotations = self.parse_tal_data(
                        tal_data, 
                        record_idx as usize, 
                        ann_idx == 0
                    )?;
                    annotations.extend(record_annotations);
                }
            }
        }
        
        // 按时间排序
        annotations.sort_by(|a, b| a.onset.cmp(&b.onset));
        
        Ok(annotations)
    }

    fn extract_timestamp(&self, data: &[u8], _record_idx: i64) -> Result<Option<i64>> {
        // 提取第一个时间戳用于验证
        let mut k = 0;
        let mut n = 0;
        let mut scratchpad = vec![0u8; 64];
        
        while k < data.len() - 1 {
            let byte = data[k];
            
            if byte == 0 {
                break;
            }
            
            if byte == 20 { // TAL分隔符
                scratchpad[n] = 0;
                let time_str = String::from_utf8_lossy(&scratchpad[0..n]);
                // 移除前导'+'号
                let time_str = time_str.trim_start_matches('+');
                if let Ok(timestamp) = time_str.parse::<f64>() {
                    return Ok(Some((timestamp * EDFLIB_TIME_DIMENSION as f64) as i64));
                }
                break;
            }
            
            if n < scratchpad.len() - 1 {
                scratchpad[n] = byte;
                n += 1;
            }
            
            k += 1;
        }
        
        Ok(None)
    }
    

    /// Parses TAL data from a byte buffer following edflib implementation
    /// 
    /// TAL format: "+<onset>[\x15<duration>]\x14<description>\x14"
    /// 
    /// This closely follows the edflib_get_annotations logic for parsing TAL data.
    fn parse_tal_data(&self, data: &[u8], _record_idx: usize, is_first_annotation_signal: bool) -> Result<Vec<Annotation>> {
        let mut annotations = Vec::new();
        let max = data.len();
        
        if max == 0 || data[max - 1] != 0 {
            return Ok(annotations);
        }
        
        // // 临时调试输出
        // println!("DEBUG: 开始解析TAL数据，长度: {}", max);
        // let preview_len = 50.min(max);
        // print!("DEBUG: 前{}字节: ", preview_len);
        // for i in 0..preview_len {
        //     let byte = data[i];
        //     if byte >= 32 && byte <= 126 {
        //         print!("{}", byte as char);
        //     } else if byte == 0x14 {
        //         print!("\\x14");
        //     } else if byte == 0x15 {
        //         print!("\\x15");
        //     } else if byte == 0 {
        //         print!("\\0");
        //     } else {
        //         print!("\\x{:02x}", byte);
        //     }
        // }
        // println!();
        
        let mut k = 0;
        let mut state = TalState::WaitingForOnset;
        let mut n = 0;
        let mut scratchpad = vec![0u8; max + 16];
        let mut time_in_txt = vec![0u8; 32];
        let mut duration_in_txt = vec![0u8; 32];
        let mut zero = 0;
        let mut annots_in_record = 0;
        let mut _annots_in_tal = 0;
        let mut duration = false;
        
        while k < max - 1 {
            let byte = data[k];
            
            // 处理null字节（TAL结束标记）
            if byte == 0 {
                if zero == 0 {
                    if k > 0 && data[k - 1] != 20 {
                        // 格式错误：null字节前应该是分隔符
                        break;
                    }
                    // 重置状态到新TAL开始
                    state = TalState::WaitingForOnset;
                    n = 0;
                    scratchpad.fill(0);
                    _annots_in_tal = 0;
                }
                zero += 1;
                k += 1;
                continue;
            }
            
            if zero > 1 {
                // 格式错误：连续的null字节太多
                break;
            }
            zero = 0;
            
            // 主状态机逻辑 - 基于edflib的布尔逻辑适应到Rust enum
            match state {
                TalState::WaitingForOnset => {
                    // 等待onset开始，跳过前导'+'
                    if byte == b'+' {
                        state = TalState::CollectingOnset;
                        n = 0;
                    } else if byte == 20 || byte == 21 {
                        // 如果没有onset就遇到分隔符，说明格式错误
                        break;
                    }
                    k += 1;
                }
                
                TalState::CollectingOnset => {
                    if byte == 20 { // Onset分隔符
                        // 完成onset收集
                        scratchpad[n] = 0;
                        let onset_str = String::from_utf8_lossy(&scratchpad[0..n]);
                        
                        // 验证onset格式
                        if !Self::is_valid_onset(&onset_str) {
                            // println!("DEBUG: 无效的onset格式: '{}'", onset_str);
                            break;
                        }
                        
                        // 保存onset时间
                        let copy_len = n.min(time_in_txt.len() - 1);
                        time_in_txt[..copy_len].copy_from_slice(&scratchpad[..copy_len]);
                        time_in_txt[copy_len] = 0;
                        
                        state = TalState::CollectingDescription;
                        n = 0;
                        
                        // println!("DEBUG: 完成onset字段: '{}'", onset_str);
                    } else if byte == 21 { // Duration分隔符
                        // 完成onset收集，开始duration
                        scratchpad[n] = 0;
                        let onset_str = String::from_utf8_lossy(&scratchpad[0..n]);
                        
                        // 验证onset格式
                        if !Self::is_valid_onset(&onset_str) {
                            // println!("DEBUG: 无效的onset格式: '{}'", onset_str);
                            break;
                        }
                        
                        // 保存onset时间
                        let copy_len = n.min(time_in_txt.len() - 1);
                        time_in_txt[..copy_len].copy_from_slice(&scratchpad[..copy_len]);
                        time_in_txt[copy_len] = 0;
                        
                        state = TalState::CollectingDuration;
                        n = 0;
                        
                        // println!("DEBUG: 完成onset字段: '{}', 开始duration", onset_str);
                    } else {
                        // 收集onset字符
                        if n < scratchpad.len() - 1 {
                            scratchpad[n] = byte;
                            n += 1;
                        }
                    }
                    k += 1;
                }
                
                TalState::CollectingDuration => {
                    if byte == 20 { // Duration分隔符（转向描述）
                        // 完成duration收集
                        scratchpad[n] = 0;
                        let duration_str = String::from_utf8_lossy(&scratchpad[0..n]);
                        
                        // 验证duration格式
                        if !Self::is_valid_duration(&duration_str) {
                            // println!("DEBUG: 无效的duration格式: '{}'", duration_str);
                            break;
                        }
                        
                        // 保存duration
                        let copy_len = n.min(duration_in_txt.len() - 1);
                        duration_in_txt[..copy_len].copy_from_slice(&scratchpad[..copy_len]);
                        duration_in_txt[copy_len] = 0;
                        
                        duration = true;
                        state = TalState::CollectingDescription;
                        n = 0;
                        
                        // println!("DEBUG: 完成duration字段: '{}'", duration_str);
                    } else if byte == 21 {
                        // 不允许在duration状态下再次遇到duration分隔符
                        // println!("DEBUG: 错误 - 多个duration字段");
                        break;
                    } else {
                        // 收集duration字符
                        if n < scratchpad.len() - 1 {
                            scratchpad[n] = byte;
                            n += 1;
                        }
                    }
                    k += 1;
                }
                
                TalState::CollectingDescription => {
                    if byte == 20 { // 描述结束分隔符
                        // 完成一个注释的收集
                        let description = if n > 0 {
                            String::from_utf8_lossy(&scratchpad[0..n]).to_string()
                        } else {
                            String::new()
                        };
                        
                        // println!("DEBUG: 描述字段结束，描述='{}', 记录中的注释数={}", description, annots_in_record);
                        
                        // 根据EDF+标准，时间戳注释（timestamp annotations）有空描述
                        // 且通常在每个数据记录的开头。用户注释即使描述为空也应该保留
                        let is_timestamp_annotation = is_first_annotation_signal && 
                                                       annots_in_record == 0 && 
                                                       description.is_empty();
                        
                        // println!("DEBUG: 是时间戳注释={}, 是第一个注释信号={}, 记录中注释数={}", 
                        //         is_timestamp_annotation, is_first_annotation_signal, annots_in_record);
                        
                        if !is_timestamp_annotation {
                            let time_str = String::from_utf8_lossy(&time_in_txt)
                                .trim_end_matches('\0').to_string();
                            
                            if let Ok(onset_seconds) = time_str.parse::<f64>() {
                                // 计算绝对时间戳
                                let onset_time = (onset_seconds * EDFLIB_TIME_DIMENSION as f64) as i64;
                                
                                // 从注释时间戳中减去文件的 starttime_offset（类似 edflib）
                                let adjusted_onset = onset_time - self.header.starttime_subsecond;
                                
                                let duration_time = if duration {
                                    let duration_str = String::from_utf8_lossy(&duration_in_txt)
                                        .trim_end_matches('\0').to_string();
                                    if let Ok(duration_seconds) = duration_str.parse::<f64>() {
                                        (duration_seconds * EDFLIB_TIME_DIMENSION as f64) as i64
                                    } else {
                                        -1
                                    }
                                } else {
                                    -1
                                };
                                
                                annotations.push(Annotation {
                                    onset: adjusted_onset,
                                    duration: duration_time,
                                    description,
                                });
                                
                                // println!("DEBUG: 添加注释 - onset={:.3}s, duration={:?}ms, 描述='{}'",
                                        // onset_seconds, 
                                        // if duration_time >= 0 { Some(duration_time as f64 / EDFLIB_TIME_DIMENSION as f64) } else { None },
                                        // annotations.last().unwrap().description);
                            } else {
                                // println!("DEBUG: 无法解析onset时间: '{}'", time_str);
                            }
                        } else {
                            // println!("DEBUG: 跳过时间戳注释");
                        }
                        
                        annots_in_record += 1;
                        _annots_in_tal += 1;
                        
                        // 重置状态变量以处理下一个注释
                        state = TalState::WaitingForOnset;
                        duration = false;
                        n = 0;
                        // 清理缓冲区
                        scratchpad.fill(0);
                        time_in_txt.fill(0);
                        duration_in_txt.fill(0);
                    } else if byte == 21 {
                        // 在描述状态下不应该遇到duration分隔符
                        break;
                    } else {
                        // 收集描述字符
                        if n < scratchpad.len() - 1 {
                            scratchpad[n] = byte;
                            n += 1;
                        }
                    }
                    k += 1;
                }
            }
        }
        
        Ok(annotations)
    }



    // 添加辅助验证函数
    fn is_valid_onset(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        
        // 注意：此时onset字符串已经不包含前导的'+'号了
        let chars: Vec<char> = s.chars().collect();
        
        // 检查是否以'.'开始或结束
        if chars[0] == '.' || chars[chars.len() - 1] == '.' {
            return false;
        }
        
        // 验证所有字符都是数字或小数点
        let mut has_dot = false;
        for ch in chars {
            if ch == '.' {
                if has_dot {
                    return false; // 不能有多个小数点
                }
                has_dot = true;
            } else if !ch.is_ascii_digit() {
                return false;
            }
        }
        
        true
    }

    fn is_valid_duration(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        
        let chars: Vec<char> = s.chars().collect();
        
        if chars[0] == '.' || chars[chars.len() - 1] == '.' {
            return false;
        }
        
        let mut has_dot = false;
        for ch in chars {
            if ch == '.' {
                if has_dot {
                    return false;
                }
                has_dot = true;
            } else if !ch.is_ascii_digit() {
                return false;
            }
        }
        
        true
    }
    
    /// 计算注释数量并解析subsecond时间（如果存在）
    fn count_annotations_and_parse_subsecond(
        reader: &mut BufReader<File>,
        signal_info: &[SignalInfo],
        datarecords: i64,
        record_size: usize,
        header_size: usize,
    ) -> Result<(i64, i64)> {
        let mut annotation_count = 0i64;
        let mut starttime_subsecond = 0i64;
        
        // 找到注释信号
        let annotation_signals: Vec<usize> = signal_info
            .iter()
            .enumerate()
            .filter_map(|(i, info)| if info.is_annotation { Some(i) } else { None })
            .collect();
        
        if annotation_signals.is_empty() {
            return Ok((0, 0));
        }
        
        // 只扫描前几个数据记录来计算注释（优化性能）
        let records_to_scan = datarecords.min(100); // 最多扫描100个记录
        
        for record_idx in 0..records_to_scan {
            // 定位到数据记录
            let record_offset = header_size as u64 + (record_idx as u64 * record_size as u64);
            reader.seek(SeekFrom::Start(record_offset))?;
            
            // 读取数据记录
            let mut record_data = vec![0u8; record_size];
            reader.read_exact(&mut record_data)?;
            
            // 处理每个注释信号
            for (ann_idx, &ann_signal_idx) in annotation_signals.iter().enumerate() {
                let ann_info = &signal_info[ann_signal_idx];
                
                // 使用预计算的buffer_offset而不是重新计算
                let signal_offset = ann_info.buffer_offset;
                
                // 提取注释信号数据
                let bytes_to_read = (ann_info.samples_per_record * 2) as usize;
                if signal_offset + bytes_to_read <= record_data.len() {
                    let tal_data = &record_data[signal_offset..signal_offset + bytes_to_read];
                    
                    // 解析TAL数据以计算注释 - 传递正确的注释信号索引
                    let (record_annotations, subsecond) = Self::quick_parse_tal_for_count(
                        tal_data, 
                        record_idx == 0,
                        ann_idx == 0  // 只有第一个注释信号才是 true
                    )?;
                    annotation_count += record_annotations;
                    
                    // 第一个记录可能包含subsecond信息
                    if record_idx == 0 && subsecond != 0 {
                        starttime_subsecond = subsecond;
                    }
                }
            }
        }
        
        Ok((annotation_count, starttime_subsecond))
    }

    /// 快速解析TAL数据仅用于计算注释数量和提取subsecond信息
    fn quick_parse_tal_for_count(data: &[u8], is_first_record: bool, is_first_annotation_signal: bool) -> Result<(i64, i64)> {
        let mut count = 0i64;
        let mut subsecond = 0i64;
        let max = data.len();
        
        if max == 0 || data[max - 1] != 0 {
            return Ok((0, 0));
        }
        
        let mut k = 0;
        let mut state = TalState::WaitingForOnset;
        let mut n = 0;
        let mut scratchpad = vec![0u8; max + 16];
        let mut zero = 0;
        let mut annots_in_record = 0;
        let mut _duration = false;
        
        while k < max - 1 {
            let byte = data[k];
            
            // 处理null字节（TAL结束标记）
            if byte == 0 {
                if zero == 0 {
                    if k > 0 && data[k - 1] != 20 {
                        // 格式错误：null字节前应该是分隔符
                        break;
                    }
                    // 重置状态到新TAL开始
                    state = TalState::WaitingForOnset;
                    n = 0;
                    scratchpad.fill(0);
                }
                zero += 1;
                k += 1;
                continue;
            }
            
            if zero > 1 {
                // 格式错误：连续的null字节太多
                break;
            }
            zero = 0;
            
            // 主状态机逻辑
            match state {
                TalState::WaitingForOnset => {
                    // 等待onset开始，跳过前导'+'
                    if byte == b'+' {
                        state = TalState::CollectingOnset;
                        n = 0;
                    } else if byte == 20 || byte == 21 {
                        // 如果没有onset就遇到分隔符，说明格式错误
                        break;
                    }
                    k += 1;
                }
                
                TalState::CollectingOnset => {
                    if byte == 20 { // Onset分隔符
                        // 完成onset收集
                        scratchpad[n] = 0;
                        let onset_str = String::from_utf8_lossy(&scratchpad[0..n]);
                        
                        // 验证onset格式
                        if !Self::is_valid_onset(&onset_str) {
                            break;
                        }
                        
                        state = TalState::CollectingDescription;
                        n = 0;
                    } else if byte == 21 { // Duration分隔符
                        // 完成onset收集，开始duration
                        scratchpad[n] = 0;
                        let onset_str = String::from_utf8_lossy(&scratchpad[0..n]);
                        
                        // 验证onset格式
                        if !Self::is_valid_onset(&onset_str) {
                            break;
                        }
                        
                        state = TalState::CollectingDuration;
                        n = 0;
                    } else {
                        // 收集onset字符
                        if n < scratchpad.len() - 1 {
                            scratchpad[n] = byte;
                            n += 1;
                        }
                    }
                    k += 1;
                }
                
                TalState::CollectingDuration => {
                    if byte == 20 { // Duration分隔符（转向描述）
                        // 完成duration收集
                        scratchpad[n] = 0;
                        let duration_str = String::from_utf8_lossy(&scratchpad[0..n]);
                        
                        // 验证duration格式
                        if !Self::is_valid_duration(&duration_str) {
                            break;
                        }
                        
                        _duration = true;
                        state = TalState::CollectingDescription;
                        n = 0;
                    } else if byte == 21 {
                        // 不允许在duration状态下再次遇到duration分隔符
                        break;
                    } else {
                        // 收集duration字符
                        if n < scratchpad.len() - 1 {
                            scratchpad[n] = byte;
                            n += 1;
                        }
                    }
                    k += 1;
                }
                
                TalState::CollectingDescription => {
                    if byte == 20 { // 描述结束分隔符
                        // 完成一个注释的收集，与parse_tal_data使用相同的逻辑判断时间戳注释
                        let description = if n > 0 {
                            String::from_utf8_lossy(&scratchpad[0..n]).to_string()
                        } else {
                            String::new()
                        };
                        
                        let is_timestamp_annotation = is_first_annotation_signal && 
                                                       annots_in_record == 0 && 
                                                       description.is_empty();
                        
                        if !is_timestamp_annotation {
                            count += 1;
                        }
                        
                        annots_in_record += 1;
                        
                        // 重置状态变量以处理下一个注释
                        state = TalState::WaitingForOnset;
                        _duration = false;
                        n = 0;
                    } else if byte == 21 {
                        // 在描述状态下不应该遇到duration分隔符
                        break;
                    } else {
                        // 收集描述字符
                        if n < scratchpad.len() - 1 {
                            scratchpad[n] = byte;
                            n += 1;
                        }
                    }
                    k += 1;
                }
            }
        }
        
        // 在第一个记录中尝试提取subsecond信息
        if is_first_record {
            subsecond = Self::extract_subsecond_from_tal(data);
        }
        
        Ok((count, subsecond))
    }

    /// 从TAL数据中提取subsecond时间信息
    fn extract_subsecond_from_tal(data: &[u8]) -> i64 {
        // 寻找第一个时间戳
        let mut k = 0;
        let mut n = 0;
        let mut scratchpad = vec![0u8; 64];
        
        while k < data.len() - 1 {
            let byte = data[k];
            
            if byte == 0 {
                break;
            }
            
            if byte == 20 { // TAL分隔符
                scratchpad[n] = 0;
                let time_str = String::from_utf8_lossy(&scratchpad[0..n]);
                let time_str = time_str.trim_start_matches('+');
                
                if let Ok(timestamp) = time_str.parse::<f64>() {
                    let timestamp_units = (timestamp * EDFLIB_TIME_DIMENSION as f64) as i64;
                    return timestamp_units % EDFLIB_TIME_DIMENSION;
                }
                break;
            }
            
            if n < scratchpad.len() - 1 {
                scratchpad[n] = byte;
                n += 1;
            }
            
            k += 1;
        }
        
        0
    }
}
