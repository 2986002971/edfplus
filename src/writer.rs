use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use chrono::{NaiveDate, NaiveTime, Datelike, Timelike};

use crate::types::SignalParam;
use crate::error::{EdfError, Result};
use crate::EDFLIB_TIME_DIMENSION;

/// Maximum number of annotation channels (matches edflib)
const EDFLIB_MAX_ANNOTATION_CHANNELS: usize = 64;

/// TAL (Time-stamped Annotations Lists) data size per annotation channel in bytes
/// 
/// Each annotation channel gets exactly 120 bytes in each data record to store
/// TAL data. This includes time stamps, durations, descriptions, and formatting 
/// characters. The 120-byte limit is part of the EDF+ specification.
const EDFLIB_ANNOTATION_BYTES: usize = 120;

/// Maximum effective length for annotation descriptions in the TAL format
/// 
/// **Critical Limitation**: This is the maximum number of characters that can
/// be stored in an annotation description within the EDF+ TAL format constraints.
/// 
/// - Descriptions longer than 40 characters will be **truncated** during file writing
/// - UTF-8 multi-byte characters may be split, potentially corrupting text
/// - This limit is enforced by the available space in the 120-byte TAL buffer
/// - Matches the behavior of the original edflib C library
/// 
/// # Impact on Users
/// 
/// ```rust
/// # use edfplus::{EdfWriter, Result};
/// # fn main() -> Result<()> {
/// let mut writer = EdfWriter::create("test.edf")?;
/// // ✅ This will be stored completely
/// writer.add_annotation(1.0, None, "Short event")?;
/// 
/// // ⚠️ This will be truncated to 40 chars
/// writer.add_annotation(2.0, None, "This is a very long annotation description that exceeds the limit")?;
/// // Result: "This is a very long annotation descripti"
/// # std::fs::remove_file("test.edf").ok();
/// # Ok(())
/// # }
/// ```
const EDFLIB_WRITE_MAX_ANNOTATION_LEN: usize = 40;



/// EDF+ file writer for creating European Data Format Plus files
/// 
/// The `EdfWriter` provides methods to create new EDF+ files and write
/// biosignal data with proper metadata. It ensures compliance with the
/// EDF+ specification and provides validation of signal parameters.
/// 
/// # File Creation Workflow
/// 
/// 1. Create writer with `EdfWriter::create()`
/// 2. Set patient and recording information
/// 3. Add signal definitions with `add_signal()`
/// 4. Write sample data with `write_samples()`
/// 5. Finalize the file with `finalize()`
/// 
/// # Examples
/// 
/// ## Basic EDF+ file creation
/// 
/// ```rust
/// use edfplus::{EdfWriter, SignalParam};
/// 
/// // Create new EDF+ file
/// let mut writer = EdfWriter::create("output.edf")?;
/// 
/// // Set patient information
/// writer.set_patient_info("P001", "M", "01-JAN-1990", "Test Patient")?;
/// 
/// // Define an EEG signal
/// let eeg_signal = SignalParam {
///     label: "EEG Fp1".to_string(),
///     samples_in_file: 0,  // Calculated automatically
///     physical_max: 200.0,
///     physical_min: -200.0,
///     digital_max: 32767,
///     digital_min: -32768,
///     samples_per_record: 256,  // 256 Hz sampling rate
///     physical_dimension: "uV".to_string(),
///     prefilter: "HP:0.1Hz LP:70Hz".to_string(),
///     transducer: "AgAgCl cup electrodes".to_string(),
/// };
/// 
/// writer.add_signal(eeg_signal)?;
/// 
/// // Generate and write sample data
/// let mut samples = Vec::new();
/// for i in 0..256 {
///     let t = i as f64 / 256.0;
///     let value = 50.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin();
///     samples.push(value);
/// }
/// 
/// writer.write_samples(&[samples])?;
/// writer.finalize()?;
/// 
/// # // Cleanup (hidden from docs)
/// # std::fs::remove_file("output.edf").ok();
/// # Ok::<(), edfplus::EdfError>(())
/// ```
/// 
/// ## Multi-channel recording
/// 
/// ```rust
/// use edfplus::{EdfWriter, SignalParam};
/// 
/// # use edfplus::doctest_utils::*;
/// let mut writer = EdfWriter::create("multi_channel.edf")?;
/// writer.set_patient_info("P002", "F", "15-MAR-1985", "Multi Channel Test")?;
/// 
/// // Add multiple signals
/// let signals = vec![
///     SignalParam {
///         label: "EEG C3".to_string(),
///         samples_in_file: 0,
///         physical_max: 200.0, physical_min: -200.0,
///         digital_max: 32767, digital_min: -32768,
///         samples_per_record: 256,
///         physical_dimension: "uV".to_string(),
///         prefilter: "HP:0.1Hz LP:70Hz".to_string(),
///         transducer: "AgAgCl electrodes".to_string(),
///     },
///     SignalParam {
///         label: "ECG Lead II".to_string(),
///         samples_in_file: 0,
///         physical_max: 5.0, physical_min: -5.0,
///         digital_max: 32767, digital_min: -32768,
///         samples_per_record: 256,
///         physical_dimension: "mV".to_string(),
///         prefilter: "HP:0.1Hz LP:100Hz".to_string(),
///         transducer: "Chest electrodes".to_string(),
///     },
/// ];
/// 
/// for signal in signals {
///     writer.add_signal(signal)?;
/// }
/// 
/// // Write 10 seconds of data
/// for second in 0..10 {
///     let mut eeg_samples = Vec::new();
///     let mut ecg_samples = Vec::new();
///     
///     for i in 0..256 {
///         let t = (second * 256 + i) as f64 / 256.0;
///         
///         // EEG: Alpha wave (10 Hz) with noise
///         let eeg = 30.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin()
///                 + 5.0 * (2.0 * std::f64::consts::PI * 50.0 * t).sin();
///         eeg_samples.push(eeg);
///         
///         // ECG: Heart beat pattern (60 BPM)
///         let ecg = 2.0 * (2.0 * std::f64::consts::PI * 1.0 * t).sin();
///         ecg_samples.push(ecg);
///     }
///     
///     writer.write_samples(&[eeg_samples, ecg_samples])?;
/// }
/// 
/// writer.finalize()?;
/// 
/// # // Cleanup (hidden from docs)
/// # std::fs::remove_file("multi_channel.edf").ok();
/// # Ok::<(), edfplus::EdfError>(())
/// ```
pub struct EdfWriter {
    file: BufWriter<File>,
    signals: Vec<SignalParam>,
    start_date: NaiveDate,
    start_time: NaiveTime,
    datarecord_duration: i64,
    samples_written: usize,
    header_written: bool,
    
    // EDF+ 字段
    patient_code: String,
    sex: String,
    birthdate: String,
    patient_name: String,
    patient_additional: String,
    admin_code: String,
    technician: String,
    equipment: String,
    recording_additional: String,
    
    // 注释存储
    annotations: Vec<crate::types::Annotation>,

    // 子秒开始时间
    starttime_subsecond: i64,
    
    // 多注释通道支持 (遵循edflib设计)
    nr_annot_chns: usize,                    // 注释通道数量 (默认1)
}

impl EdfWriter {
    /// Creates a new EDF+ file writer
    /// 
    /// Opens a new file for writing and initializes the writer with default
    /// values. The file will be created (or truncated if it exists).
    /// 
    /// # Arguments
    /// 
    /// * `path` - Path where the EDF+ file should be created
    /// 
    /// # Returns
    /// 
    /// Returns a `Result<EdfWriter, EdfError>`. On success, contains an `EdfWriter`
    /// ready for configuration and data writing.
    /// 
    /// # Errors
    /// 
    /// * `EdfError::FileNotFound` - Cannot create file (permission issues, invalid path, etc.)
    /// 
    /// # Default Values
    /// 
    /// The writer is initialized with the following defaults:
    /// - Start date: January 1, 1985
    /// - Start time: 00:00:00
    /// - All patient and recording fields set to "X" (anonymized)
    /// - Data record duration: 1 second
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::EdfWriter;
    /// 
    /// // Create a new EDF+ file
    /// let writer = EdfWriter::create("new_recording.edf")?;
    /// println!("EDF+ writer created successfully");
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("new_recording.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    /// 
    /// ## Handling creation errors
    /// 
    /// ```rust
    /// use edfplus::EdfWriter;
    /// 
    /// match EdfWriter::create("/invalid/path/file.edf") {
    ///     Ok(_) => println!("File created"),
    ///     Err(e) => eprintln!("Failed to create file: {}", e),
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::create(&path)
            .map_err(|e| EdfError::FileNotFound(format!("{}: {}", path.as_ref().display(), e)))?;
        
        let writer = BufWriter::new(file);
        
        // 使用默认日期时间
        let default_date = NaiveDate::from_ymd_opt(1985, 1, 1).unwrap();
        let default_time = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        
        Ok(EdfWriter {
            file: writer,
            signals: Vec::new(),
            start_date: default_date,
            start_time: default_time,
            datarecord_duration: EDFLIB_TIME_DIMENSION, // 1秒
            samples_written: 0,
            header_written: false,
            patient_code: "X".to_string(),
            sex: "X".to_string(),
            birthdate: "X".to_string(),
            patient_name: "X".to_string(),
            patient_additional: "X".to_string(),
            admin_code: "X".to_string(),
            technician: "X".to_string(),
            equipment: "X".to_string(),
            recording_additional: "X".to_string(),
            annotations: Vec::new(),
            starttime_subsecond: 0,
            nr_annot_chns: 1,  // 默认1个注释通道
        })
    }
    
    /// Adds a signal definition to the EDF+ file
    /// 
    /// Each signal represents a data channel (e.g., EEG electrode, ECG lead).
    /// Signals must be added before writing any data. The order in which
    /// signals are added determines their index for data writing.
    /// 
    /// # Arguments
    /// 
    /// * `signal` - SignalParam containing all signal metadata
    /// 
    /// # Errors
    /// 
    /// * `EdfError::InvalidFormat` - Trying to add signal after header is written
    /// * `EdfError::PhysicalMinEqualsMax` - Invalid physical range
    /// * `EdfError::DigitalMinEqualsMax` - Invalid digital range
    /// 
    /// # Signal Parameter Requirements
    /// 
    /// - `physical_min` must be different from `physical_max`
    /// - `digital_min` must be different from `digital_max`
    /// - `samples_per_record` should match the intended sampling rate
    /// - `label` should be descriptive and follow EDF+ conventions
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::{EdfWriter, SignalParam};
    /// 
    /// let mut writer = EdfWriter::create("signals.edf")?;
    /// 
    /// // Add an EEG signal
    /// let eeg_signal = SignalParam {
    ///     label: "EEG Fp1-A1".to_string(),
    ///     samples_in_file: 0,  // Will be calculated
    ///     physical_max: 200.0,    // +200 µV
    ///     physical_min: -200.0,   // -200 µV  
    ///     digital_max: 32767,     // 16-bit signed max
    ///     digital_min: -32768,    // 16-bit signed min
    ///     samples_per_record: 256, // 256 Hz sampling
    ///     physical_dimension: "uV".to_string(),
    ///     prefilter: "HP:0.1Hz LP:70Hz N:50Hz".to_string(),
    ///     transducer: "AgAgCl cup electrodes".to_string(),
    /// };
    /// 
    /// writer.add_signal(eeg_signal)?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("signals.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    /// 
    /// ## Adding multiple signals with different sampling rates
    /// 
    /// ```rust
    /// use edfplus::{EdfWriter, SignalParam};
    /// 
    /// let mut writer = EdfWriter::create("mixed_rates.edf")?;
    /// 
    /// // High-frequency EEG signal
    /// writer.add_signal(SignalParam {
    ///     label: "EEG C3-A1".to_string(),
    ///     samples_in_file: 0,
    ///     physical_max: 200.0, physical_min: -200.0,
    ///     digital_max: 32767, digital_min: -32768,
    ///     samples_per_record: 512,  // 512 Hz
    ///     physical_dimension: "uV".to_string(),
    ///     prefilter: "HP:0.1Hz LP:200Hz".to_string(),
    ///     transducer: "Gold cup electrodes".to_string(),
    /// })?;
    /// 
    /// // Lower-frequency physiological signal
    /// writer.add_signal(SignalParam {
    ///     label: "Temperature".to_string(),
    ///     samples_in_file: 0,
    ///     physical_max: 40.0, physical_min: 30.0,
    ///     digital_max: 32767, digital_min: -32768,
    ///     samples_per_record: 1,    // 1 Hz
    ///     physical_dimension: "degC".to_string(),
    ///     prefilter: "None".to_string(),
    ///     transducer: "Thermistor".to_string(),
    /// })?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("mixed_rates.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    pub fn add_signal(&mut self, signal: SignalParam) -> Result<()> {
        if self.header_written {
            return Err(EdfError::InvalidFormat("Cannot add signal after writing header".to_string()));
        }
        
        // 验证信号参数
        if signal.physical_min == signal.physical_max {
            return Err(EdfError::PhysicalMinEqualsMax);
        }
        if signal.digital_min == signal.digital_max {
            return Err(EdfError::DigitalMinEqualsMax);
        }
        
        self.signals.push(signal);
        Ok(())
    }
    
    /// Sets patient information for the EDF+ file
    /// 
    /// Patient information is embedded in the EDF+ header and follows
    /// specific formatting requirements. This information is crucial
    /// for medical applications but can be anonymized for privacy.
    /// 
    /// # Arguments
    /// 
    /// * `code` - Patient identification code (max 80 chars)
    /// * `sex` - Patient sex: "M", "F", or "X" (unknown)
    /// * `birthdate` - Birth date in DD-MMM-YYYY format or "X"
    /// * `name` - Patient name or "X" for anonymized data
    /// 
    /// # Errors
    /// 
    /// * `EdfError::InvalidFormat` - Trying to modify after header written
    /// 
    /// # Format Requirements
    /// 
    /// - Patient code should be unique and meaningful
    /// - Sex must be "M", "F", or "X"
    /// - Birth date format: "02-MAY-1951" or "X" if unknown
    /// - Name can be full name or "X" for anonymization
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::EdfWriter;
    /// 
    /// let mut writer = EdfWriter::create("patient_data.edf")?;
    /// 
    /// // Set complete patient information
    /// writer.set_patient_info(
    ///     "P001-2024",           // Patient code
    ///     "F",                   // Female
    ///     "15-MAR-1990",         // Birth date
    ///     "Jane Doe"             // Patient name
    /// )?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("patient_data.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    /// 
    /// ## Anonymized patient data
    /// 
    /// ```rust
    /// use edfplus::EdfWriter;
    /// 
    /// let mut writer = EdfWriter::create("anonymous.edf")?;
    /// 
    /// // Anonymized information for privacy protection
    /// writer.set_patient_info(
    ///     "ANON-001",    // Anonymous code
    ///     "X",           // Sex unknown/anonymized
    ///     "X",           // Birth date anonymized
    ///     "X"            // Name anonymized
    /// )?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("anonymous.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    /// 
    /// ## Research study format
    /// 
    /// ```rust
    /// use edfplus::EdfWriter;
    /// 
    /// let mut writer = EdfWriter::create("study_subject.edf")?;
    /// 
    /// // Research study patient coding
    /// writer.set_patient_info(
    ///     "STUDY-EEG-S042",      // Study-specific ID
    ///     "M",                   // Male
    ///     "22-JUL-1985",         // Known birth date
    ///     "Subject 042"          // Study identifier
    /// )?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("study_subject.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    pub fn set_patient_info(&mut self, code: &str, sex: &str, birthdate: &str, name: &str) -> Result<()> {
        if self.header_written {
            return Err(EdfError::InvalidFormat("Cannot modify patient info after writing header".to_string()));
        }
        
        self.patient_code = code.to_string();
        self.sex = sex.to_string();
        self.birthdate = birthdate.to_string();
        self.patient_name = name.to_string();
        Ok(())
    }
    
    /// Sets the data record duration for the EDF+ file
    /// 
    /// The data record duration determines how long each data record represents
    /// in time. This affects the temporal resolution and file organization.
    /// Most EDF+ files use 1 second data records, but other durations are possible.
    /// 
    /// # Arguments
    /// 
    /// * `duration_seconds` - Duration of each data record in seconds
    /// 
    /// # Errors
    /// 
    /// * `EdfError::InvalidFormat` - Trying to modify after header written
    /// * `EdfError::InvalidArgument` - Duration <= 0 or too large
    /// 
    /// # Common Values
    /// 
    /// - 1.0 seconds: Standard for most clinical recordings
    /// - 0.1 seconds: Higher temporal resolution for fast events
    /// - 10.0 seconds: Lower resolution for long-term monitoring
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::EdfWriter;
    /// 
    /// let mut writer = EdfWriter::create("custom_duration.edf")?;
    /// 
    /// // Set 0.5 second data records for higher temporal resolution
    /// writer.set_datarecord_duration(0.5)?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("custom_duration.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    /// 
    /// ## High-frequency recording
    /// 
    /// ```rust
    /// use edfplus::EdfWriter;
    /// 
    /// let mut writer = EdfWriter::create("high_freq.edf")?;
    /// 
    /// // Use 0.1 second records for fast neural signals
    /// writer.set_datarecord_duration(0.1)?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("high_freq.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    pub fn set_datarecord_duration(&mut self, duration_seconds: f64) -> Result<()> {
        if self.header_written {
            return Err(EdfError::InvalidFormat("Cannot modify data record duration after writing header".to_string()));
        }
        
        if duration_seconds <= 0.0 || duration_seconds > 3600.0 {
            return Err(EdfError::InvalidFormat("Data record duration must be between 0 and 3600 seconds".to_string()));
        }
        
        // Convert seconds to EDFLIB_TIME_DIMENSION units (100 nanoseconds)
        self.datarecord_duration = (duration_seconds * EDFLIB_TIME_DIMENSION as f64) as i64;
        Ok(())
    }

    /// 写入头部
    fn write_header(&mut self, total_datarecords: i64) -> Result<()> {
        if self.header_written {
            return Ok(());
        }
        
        // 创建注释信号 - 支持多个注释通道
        let mut annotation_signals = Vec::new();
        let annotation_bytes_per_record = EDFLIB_ANNOTATION_BYTES; // 每个注释通道120字节
        let annotation_samples_per_record = annotation_bytes_per_record / 2; // 每样本2字节
        
        for _ in 0..self.nr_annot_chns {
            annotation_signals.push(SignalParam {
                label: "EDF Annotations ".to_string(), // 标准要求的标签
                samples_in_file: total_datarecords * annotation_samples_per_record as i64,
                physical_max: 1.0,
                physical_min: -1.0,
                digital_max: 32767,
                digital_min: -32768,
                samples_per_record: annotation_samples_per_record as i32,
                physical_dimension: "".to_string(),
                prefilter: "".to_string(),
                transducer: "".to_string(),
            });
        }
        
        let total_signals = self.signals.len() + self.nr_annot_chns;
        let header_size = (total_signals + 1) * 256;
        
        // 写入主头部 (256字节) - 按照edflib格式
        let mut main_header = vec![0u8; 256];
        
        // 版本 (8字节)
        main_header[0..8].copy_from_slice(b"0       ");
        
        // 患者信息字段 (80字节)
        let patient_field = format!("{} {} {} {} {}", 
            self.patient_code, self.sex, self.birthdate, self.patient_name, self.patient_additional);
        let patient_bytes = patient_field.as_bytes();
        let patient_len = patient_bytes.len().min(80);
        main_header[8..8+patient_len].copy_from_slice(&patient_bytes[..patient_len]);
        
        // 记录信息字段 (80字节)
        let recording_field = format!("Startdate {} {} {} {} {}", 
            self.start_date.format("%d-%b-%Y"), self.admin_code, self.technician, 
            self.equipment, self.recording_additional);
        let recording_bytes = recording_field.as_bytes();
        let recording_len = recording_bytes.len().min(80);
        main_header[88..88+recording_len].copy_from_slice(&recording_bytes[..recording_len]);
        
        // 开始日期 (8字节) "dd.mm.yy"
        let date_str = format!("{:02}.{:02}.{:02}", 
            self.start_date.day(), self.start_date.month(), self.start_date.year() % 100);
        main_header[168..176].copy_from_slice(date_str.as_bytes());
        
        // 开始时间 (8字节) "hh.mm.ss"
        let time_str = format!("{:02}.{:02}.{:02}", 
            self.start_time.hour(), self.start_time.minute(), self.start_time.second());
        main_header[176..184].copy_from_slice(time_str.as_bytes());
        
        // 头部大小 (8字节)
        let header_size_str = format!("{:<8}", header_size);
        main_header[184..192].copy_from_slice(header_size_str.as_bytes());
        
        // EDF+标识 (44字节)
        main_header[192..197].copy_from_slice(b"EDF+C");
        
        // 数据记录数 (8字节)
        let datarecords_str = format!("{:<8}", total_datarecords);
        main_header[236..244].copy_from_slice(datarecords_str.as_bytes());
        
        // 数据记录持续时间 (8字节)
        let duration_seconds = self.datarecord_duration as f64 / EDFLIB_TIME_DIMENSION as f64;
        let duration_str = format!("{:<8}", duration_seconds);
        main_header[244..252].copy_from_slice(duration_str.as_bytes());
        
        // 信号数 (4字节)
        let signals_str = format!("{:<4}", total_signals);
        main_header[252..256].copy_from_slice(signals_str.as_bytes());
        
        self.file.write_all(&main_header)?;
        
        // 写入信号头部 - 根据注释信号位置确定顺序
        self.write_signal_headers_with_annotations(&annotation_signals)?;
        
        self.header_written = true;
        Ok(())
    }
    
    /// Writes sample data for all signals to the current data record
    /// 
    /// **⚠️ WARNING: IRREVERSIBLE OPERATION**
    /// 
    /// Once this method is called, the written data record **CANNOT be modified**.
    /// This library uses a **sequential streaming write** architecture that does not
    /// support backtracking or random access modification.
    /// 
    /// **What happens when you call this method:**
    /// 1. Signal sample data is immediately written to the file buffer
    /// 2. Annotation data for this time period is generated and written
    /// 3. The internal record counter is incremented
    /// 4. **The written content becomes immutable**
    /// 
    /// **If you need to modify data:**
    /// - Collect all your data and annotations first
    /// - Create a new file with the corrected data
    /// - See documentation for strategies: in-memory preparation, temporary files, etc.
    /// 
    /// # Arguments
    /// 
    /// * `samples` - Vector of sample vectors, one per signal channel
    ///   - Must contain exactly the same number of vectors as signals added
    ///   - Each vector must contain exactly `samples_per_record` samples
    /// 
    /// # Errors
    /// 
    /// * `EdfError::InvalidFormat` - Wrong number of sample vectors or samples per vector
    /// * `EdfError::FileWriteError` - I/O error during writing
    /// * `EdfError::NotReady` - File headers not written yet
    /// 
    /// # Sample Organization
    /// 
    /// The `samples` parameter must be organized as:
    /// - Outer vector: one element per signal (in order added)
    /// - Inner vectors: physical values for each signal
    /// - All inner vectors must have the same length (matching `samples_per_record`)
    /// 
    /// # Examples
    /// 
    /// ## Writing a single data record with multiple signals
    /// 
    /// ```rust
    /// use edfplus::{EdfWriter, SignalParam};
    /// 
    /// let mut writer = EdfWriter::create("multi_signal.edf")?;
    /// 
    /// // Add two signals with the same sampling rate
    /// writer.add_signal(SignalParam {
    ///     label: "EEG Fp1".to_string(),
    ///     samples_in_file: 0,
    ///     physical_max: 100.0, physical_min: -100.0,
    ///     digital_max: 32767, digital_min: -32768,
    ///     samples_per_record: 256,  // 256 samples per data record
    ///     physical_dimension: "uV".to_string(),
    ///     prefilter: "HP:0.1Hz LP:70Hz".to_string(),
    ///     transducer: "AgAgCl electrodes".to_string(),
    /// })?;
    /// 
    /// writer.add_signal(SignalParam {
    ///     label: "ECG Lead II".to_string(),
    ///     samples_in_file: 0,
    ///     physical_max: 5.0, physical_min: -5.0,
    ///     digital_max: 32767, digital_min: -32768,
    ///     samples_per_record: 256,  // Same sampling rate as EEG
    ///     physical_dimension: "mV".to_string(),
    ///     prefilter: "HP:0.1Hz LP:100Hz".to_string(),
    ///     transducer: "Chest electrodes".to_string(),
    /// })?;
    /// 
    /// // Generate sample data (256 samples for each signal)
    /// let mut eeg_samples = Vec::new();
    /// let mut ecg_samples = Vec::new();
    /// 
    /// for i in 0..256 {
    ///     let t = i as f64 / 256.0;  // Time within this 1-second data record
    ///     eeg_samples.push(20.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin()); // 10 Hz EEG
    ///     ecg_samples.push(1.0 * (2.0 * std::f64::consts::PI * 1.0 * t).sin());   // 1 Hz ECG
    /// }
    /// 
    /// // Write one data record containing both signals
    /// writer.write_samples(&[eeg_samples, ecg_samples])?;
    /// writer.finalize()?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("multi_signal.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    /// 
    /// ## Writing multiple data records (continuous recording)
    /// 
    /// ```rust
    /// use edfplus::{EdfWriter, SignalParam};
    /// 
    /// let mut writer = EdfWriter::create("continuous.edf")?;
    /// 
    /// // Add a single signal
    /// writer.add_signal(SignalParam {
    ///     label: "Continuous EEG".to_string(),
    ///     samples_in_file: 0,
    ///     physical_max: 100.0, physical_min: -100.0,
    ///     digital_max: 32767, digital_min: -32768,
    ///     samples_per_record: 256,  // 256 Hz sampling rate (256 samples per 1-second record)
    ///     physical_dimension: "uV".to_string(),
    ///     prefilter: "HP:0.1Hz LP:70Hz".to_string(),
    ///     transducer: "AgAgCl electrodes".to_string(),
    /// })?;
    /// 
    /// // Write 10 seconds of continuous data (10 data records)
    /// for second in 0..10 {
    ///     let mut samples = Vec::new();
    ///     
    ///     // Generate 256 samples for this 1-second data record
    ///     for i in 0..256 {
    ///         let t = (second * 256 + i) as f64 / 256.0;  // Absolute time since recording start
    ///         let value = 50.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin();
    ///         samples.push(value);
    ///     }
    ///     
    ///     // Write one data record (note: samples is a Vec<f64>, so we wrap it in &[samples])
    ///     writer.write_samples(&[samples])?;
    /// }
    /// 
    /// writer.finalize()?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("continuous.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    /// 
    /// ## Writing multiple signals with different sampling rates
    /// 
    /// ```rust
    /// use edfplus::{EdfWriter, SignalParam};
    /// 
    /// let mut writer = EdfWriter::create("mixed_rates.edf")?;
    /// 
    /// // High-frequency EEG signal
    /// writer.add_signal(SignalParam {
    ///     label: "EEG C3".to_string(),
    ///     samples_in_file: 0,
    ///     physical_max: 200.0, physical_min: -200.0,
    ///     digital_max: 32767, digital_min: -32768,
    ///     samples_per_record: 500,  // 500 Hz sampling rate
    ///     physical_dimension: "uV".to_string(),
    ///     prefilter: "HP:0.1Hz LP:200Hz".to_string(),
    ///     transducer: "Gold cup electrodes".to_string(),
    /// })?;
    /// 
    /// // Lower-frequency physiological signal
    /// writer.add_signal(SignalParam {
    ///     label: "Respiration".to_string(),
    ///     samples_in_file: 0,
    ///     physical_max: 10.0, physical_min: -10.0,
    ///     digital_max: 32767, digital_min: -32768,
    ///     samples_per_record: 25,   // 25 Hz sampling rate
    ///     physical_dimension: "arbitrary".to_string(),
    ///     prefilter: "LP:10Hz".to_string(),
    ///     transducer: "Strain gauge".to_string(),
    /// })?;
    /// 
    /// // Write 5 seconds of data
    /// for second in 0..5 {
    ///     // EEG: 500 samples for this data record
    ///     let mut eeg_samples = Vec::new();
    ///     for i in 0..500 {
    ///         let t = (second * 500 + i) as f64 / 500.0;
    ///         let value = 100.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin();
    ///         eeg_samples.push(value);
    ///     }
    ///     
    ///     // Respiration: 25 samples for this data record
    ///     let mut resp_samples = Vec::new();
    ///     for i in 0..25 {
    ///         let t = (second * 25 + i) as f64 / 25.0;
    ///         let value = 5.0 * (2.0 * std::f64::consts::PI * 0.3 * t).sin(); // 0.3 Hz breathing
    ///         resp_samples.push(value);
    ///     }
    ///     
    ///     // Write both signals for this data record
    ///     writer.write_samples(&[eeg_samples, resp_samples])?;
    /// }
    /// 
    /// writer.finalize()?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("mixed_rates.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    pub fn write_samples(&mut self, samples: &[Vec<f64>]) -> Result<()> {
        if samples.len() != self.signals.len() {
            return Err(EdfError::InvalidFormat("Sample count must match signal count".to_string()));
        }
        
        // 验证每个信号的样本数
        for (i, signal_samples) in samples.iter().enumerate() {
            let expected_samples = self.signals[i].samples_per_record as usize;
            if signal_samples.len() != expected_samples {
                return Err(EdfError::InvalidFormat(
                    format!("Signal {} expected {} samples per record, got {}", 
                           i, expected_samples, signal_samples.len())
                ));
            }
        }
        
        // 如果还没写头部，先写头部
        if !self.header_written {
            self.write_header(1)?; // 临时使用1，会在finalize时更新
        }
        
        // 按照edflib的顺序写入数据：信号数据 + 注释信号
    
        // 写入所有信号的样本数据
        for signal_idx in 0..self.signals.len() {
            let signal = &self.signals[signal_idx];
            let signal_samples = &samples[signal_idx];
            
            for &physical_value in signal_samples {
                let digital_value = signal.to_digital(physical_value);
                
                // 应用范围限制
                let clamped_value = digital_value
                    .max(signal.digital_min)
                    .min(signal.digital_max);
                
                // 写入为16位小端序
                let bytes = (clamped_value as i16).to_le_bytes();
                self.file.write_all(&bytes)?;
            }
        }
        
        // 写入注释信号的TAL数据 - 支持多个注释通道
        for channel_idx in 0..self.nr_annot_chns {
            let annotation_data = self.generate_annotation_tal_for_channel(self.samples_written, channel_idx)?;
            self.file.write_all(&annotation_data)?;
        }
        
        self.samples_written += 1;
        Ok(())
    }
    
    /// Finalizes the EDF+ file and closes it
    /// 
    /// This method must be called to complete the file writing process.
    /// It flushes any remaining data to disk and properly closes the file.
    /// After calling this method, the writer is consumed and cannot be used again.
    /// 
    /// # Errors
    /// 
    /// * `EdfError::FileWriteError` - I/O error during file finalization
    /// 
    /// # File Integrity
    /// 
    /// Failing to call `finalize()` may result in:
    /// - Incomplete file headers
    /// - Missing data records
    /// - Corrupted file structure
    /// 
    /// Always call `finalize()` when finished writing data.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::{EdfWriter, SignalParam};
    /// 
    /// let mut writer = EdfWriter::create("final_test.edf")?;
    /// 
    /// // Add signal and write data...
    /// writer.add_signal(SignalParam {
    ///     label: "Test Signal".to_string(),
    ///     samples_in_file: 0,
    ///     physical_max: 1.0, physical_min: -1.0,
    ///     digital_max: 32767, digital_min: -32768,
    ///     samples_per_record: 10,
    ///     physical_dimension: "V".to_string(),
    ///     prefilter: "None".to_string(),
    ///     transducer: "Test".to_string(),
    /// })?;
    /// 
    /// let samples = vec![0.1, 0.2, 0.3, 0.4, 0.5, -0.1, -0.2, -0.3, -0.4, -0.5];
    /// writer.write_samples(&[samples])?;
    /// 
    /// // Always finalize to ensure file integrity
    /// writer.finalize()?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("final_test.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    /// 
    /// ## Error handling during finalization
    /// 
    /// ```rust
    /// use edfplus::{EdfWriter, SignalParam};
    /// 
    /// fn test_finalize() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut writer = EdfWriter::create("error_test.edf")?;
    ///     // ... add signals and write data ...
    ///     
    ///     match writer.finalize() {
    ///         Ok(()) => println!("File successfully completed"),
    ///         Err(e) => {
    ///             eprintln!("Error finalizing file: {}", e);
    ///             // File may be corrupted
    ///         }
    ///     }
    ///     
    ///     # // Cleanup (hidden from docs)
    ///     # std::fs::remove_file("error_test.edf").ok();
    ///     Ok(())
    /// }
    /// 
    /// # test_finalize().unwrap();
    /// ```
    pub fn finalize(mut self) -> Result<()> {
        // 如果有数据写入但头部记录数不正确，需要更新头部
        if self.header_written && self.samples_written > 1 {
            use std::io::{Seek, SeekFrom};
            
            // 刷新缓冲区以确保所有数据都写入了
            self.file.flush()?;
            
            // 获取内部文件引用并seek到数据记录数位置 (236-244字节)
            let mut file = self.file.into_inner().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            file.seek(SeekFrom::Start(236))?;
            
            // 更新数据记录数
            let datarecords_str = format!("{:<8}", self.samples_written);
            file.write_all(datarecords_str.as_bytes())?;
            
            // 确保数据写入磁盘
            file.flush()?;
        } else {
            self.file.flush()?;
        }
        Ok(())
    }
    
    /// Adds an annotation/event to the EDF+ file
    /// 
    /// **⚠️ CRITICAL TIMING CONSTRAINT**
    /// 
    /// Annotations are only saved when their onset time falls within **future data records**.
    /// Once a data record is written with `write_samples()`, no new annotations can be 
    /// added to that time period.
    /// 
    /// **Timing Rules:**
    /// - Add annotations **BEFORE** writing the data records that cover their time range
    /// - Annotations with `onset_seconds` in already-written time periods will be **silently lost**
    /// - This is due to the sequential write architecture - no backtracking is possible
    /// 
    /// # Arguments
    /// 
    /// * `onset_seconds` - Time when the event occurred (seconds since recording start)
    /// * `duration_seconds` - Duration of the event in seconds (None for instantaneous events)  
    /// * `description` - UTF-8 text describing the event (max 40 chars effective)
    /// 
    /// # Important Limitations
    /// 
    /// ## Description Length Limit
    /// 
    /// **Warning**: Annotation descriptions are subject to EDF+ format constraints:
    /// - Maximum effective length is **40 characters** in the final TAL (Time-stamped Annotations Lists) data
    /// - Longer descriptions will be **automatically truncated** during file writing
    /// - UTF-8 multi-byte characters may be truncated at byte boundaries, potentially corrupting the text
    /// - This limit is enforced by the EDF+ standard and matches edflib behavior
    /// 
    /// ```rust
    /// # use edfplus::{EdfWriter, SignalParam, Result};
    /// # fn main() -> Result<()> {
    /// let mut writer = EdfWriter::create("annotations.edf")?;
    /// // ✅ Good - within 40 character limit
    /// writer.add_annotation(1.0, None, "Sleep stage N2")?;
    /// 
    /// // ⚠️  Warning - will be truncated to 40 chars
    /// writer.add_annotation(2.0, None, "This is a very long annotation description that exceeds the EDF+ limit")?;
    /// // Result: "This is a very long annotation descripti"
    /// # std::fs::remove_file("annotations.edf").ok();
    /// # Ok(())
    /// # }
    /// ```
    /// 
    /// ## Time Range Constraints
    /// 
    /// **Critical**: Annotations are only saved if their onset time falls within written data records:
    /// - Annotations with `onset_seconds` >= total file duration will be **silently discarded**
    /// - Each data record covers a specific time range (typically 1 second)
    /// - An annotation at time T is only saved if there's a data record covering [T, T+duration)
    /// 
    /// ```rust
    /// // Write 5 seconds of data (5 records)
    /// # use edfplus::{EdfWriter, SignalParam, Result};
    /// # fn main() -> Result<()> {
    /// let mut writer = EdfWriter::create("annotations.edf")?;
    /// # let signal = SignalParam {
    /// #     label: "EEG".to_string(),
    /// #     samples_in_file: 0,
    /// #     physical_max: 100.0,
    /// #     physical_min: -100.0,
    /// #     digital_max: 32767,
    /// #     digital_min: -32768,
    /// #     samples_per_record: 256,
    /// #     physical_dimension: "uV".to_string(),
    /// #     prefilter: "".to_string(),
    /// #     transducer: "".to_string(),
    /// # };
    /// # writer.add_signal(signal)?;
    /// for i in 0..5 {
    ///     let samples = vec![0.0; 256];
    ///     writer.write_samples(&[samples])?;
    /// }
    /// 
    /// // ✅ Good - within file duration [0.0, 5.0)
    /// writer.add_annotation(2.5, None, "Valid event")?;
    /// writer.add_annotation(4.999, None, "Last moment")?;
    /// 
    /// // ❌ Lost - outside file duration
    /// writer.add_annotation(5.0, None, "Will be discarded")?;
    /// writer.add_annotation(6.0, None, "Also discarded")?;
    /// # std::fs::remove_file("annotations.edf").ok();
    /// # Ok(())
    /// # }
    /// ```
    /// 
    /// ## Best Practices
    /// 
    /// 1. **Keep descriptions concise** (≤40 characters)
    /// 2. **Add annotations before finalizing** the file
    /// 3. **Ensure sufficient data records** cover all annotation times
    /// 4. **Use ASCII characters** when possible to avoid UTF-8 truncation issues
    /// 5. **Validate annotation times** against your data duration
    /// 
    /// # Time Precision
    /// 
    /// Time values are internally stored with 100-nanosecond precision.
    /// Input values will be rounded to the nearest 100 nanoseconds.
    /// 
    /// # Errors
    /// 
    /// Returns `EdfError::InvalidFormat` if:
    /// - `onset_seconds` is negative
    /// - `duration_seconds` is negative
    /// - `description` is empty
    /// - `description` exceeds 512 characters (pre-truncation validation)
    /// 
    /// # Examples
    /// 
    /// ## Basic Usage
    /// 
    /// ```rust
    /// use edfplus::{EdfWriter, SignalParam};
    /// # use std::fs;
    /// 
    /// let mut writer = EdfWriter::create("annotations_test.edf")?;
    /// writer.set_patient_info("P001", "M", "01-JAN-1990", "Test Patient")?;
    /// 
    /// // Add a signal
    /// let signal = SignalParam {
    ///     label: "EEG".to_string(),
    ///     samples_in_file: 0,
    ///     physical_max: 100.0,
    ///     physical_min: -100.0,
    ///     digital_max: 32767,
    ///     digital_min: -32768,
    ///     samples_per_record: 256,
    ///     physical_dimension: "uV".to_string(),
    ///     prefilter: "HP:0.1Hz".to_string(),
    ///     transducer: "AgAgCl".to_string(),
    /// };
    /// writer.add_signal(signal)?;
    /// 
    /// // Write some data FIRST to establish time range
    /// for i in 0..10 {
    ///     let samples = vec![10.0; 256];
    ///     writer.write_samples(&[samples])?;  // Creates 10 seconds of data
    /// }
    /// 
    /// // Add annotations within the data time range [0.0, 10.0)
    /// writer.add_annotation(0.5, None, "Recording start")?;
    /// writer.add_annotation(2.0, Some(1.0), "Sleep stage 1")?;
    /// writer.add_annotation(5.5, None, "Eye movement")?;
    /// writer.add_annotation(9.999, None, "Near end")?;  // Still within range
    /// 
    /// writer.finalize()?;
    /// 
    /// # // Cleanup
    /// # std::fs::remove_file("annotations_test.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    /// 
    /// ## Sleep Study Example with Proper Time Management
    /// 
    /// ```rust
    /// use edfplus::{EdfWriter, SignalParam};
    /// # use std::fs;
    /// 
    /// let mut writer = EdfWriter::create("sleep_study.edf")?;
    /// writer.set_patient_info("S001", "F", "15-MAR-1980", "Sleep Study")?;
    /// 
    /// let eeg_signal = SignalParam {
    ///     label: "C3-A2".to_string(),
    ///     samples_in_file: 0,
    ///     physical_max: 100.0,
    ///     physical_min: -100.0,
    ///     digital_max: 32767,
    ///     digital_min: -32768,
    ///     samples_per_record: 100,  // 100 Hz
    ///     physical_dimension: "uV".to_string(),
    ///     prefilter: "0.1-35Hz".to_string(),
    ///     transducer: "AgAgCl".to_string(),
    /// };
    /// writer.add_signal(eeg_signal)?;
    /// 
    /// // Record 30 minutes (1800 seconds) of sleep data
    /// let recording_duration_seconds = 1800;
    /// for second in 0..recording_duration_seconds {
    ///     let mut samples = Vec::with_capacity(100);
    ///     for sample_idx in 0..100 {
    ///         let t = second as f64 + (sample_idx as f64 / 100.0);
    ///         let eeg_value = 20.0 * (2.0 * std::f64::consts::PI * 10.0 * t).sin();
    ///         samples.push(eeg_value);
    ///     }
    ///     writer.write_samples(&[samples])?;
    /// }
    /// 
    /// // Now add sleep annotations - all within [0, 1800) seconds
    /// writer.add_annotation(300.0, None, "Lights out")?;                // 5 min
    /// writer.add_annotation(480.0, None, "Sleep onset")?;               // 8 min  
    /// writer.add_annotation(600.0, Some(1200.0), "Stage N2")?;          // 10-30 min
    /// writer.add_annotation(900.0, None, "Sleep spindle")?;             // 15 min
    /// writer.add_annotation(1200.0, Some(300.0), "REM episode")?;       // 20-25 min
    /// writer.add_annotation(1790.0, None, "Wake up")?;                  // 29:50 - still valid
    /// 
    /// writer.finalize()?;
    /// 
    /// # // Cleanup  
    /// # std::fs::remove_file("sleep_study.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    pub fn add_annotation(&mut self, onset_seconds: f64, duration_seconds: Option<f64>, description: &str) -> Result<()> {
        // Validate inputs
        if onset_seconds < 0.0 {
            return Err(EdfError::InvalidFormat("Annotation onset cannot be negative".to_string()));
        }
        
        if let Some(duration) = duration_seconds {
            if duration < 0.0 {
                return Err(EdfError::InvalidFormat("Annotation duration cannot be negative".to_string()));
            }
        }
        
        if description.is_empty() {
            return Err(EdfError::InvalidFormat("Annotation description cannot be empty".to_string()));
        }
        
        if description.len() > 512 {
            return Err(EdfError::InvalidFormat("Annotation description too long (max 512 characters)".to_string()));
        }
        
        // Convert to internal time units (100 nanoseconds)
        let onset = (onset_seconds * EDFLIB_TIME_DIMENSION as f64) as i64;
        let duration = duration_seconds
            .map(|d| (d * EDFLIB_TIME_DIMENSION as f64) as i64)
            .unwrap_or(-1);
        
        // Create and store annotation
        let annotation = crate::types::Annotation {
            onset,
            duration,
            description: description.to_string(),
        };
        
        self.annotations.push(annotation);
        Ok(())
    }

    /// Gets the current number of annotations
    /// 
    /// This can be useful for tracking how many annotations have been added
    /// before finalizing the file.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::{EdfWriter, SignalParam};
    /// # use std::fs;
    /// 
    /// let mut writer = EdfWriter::create("count_test.edf")?;
    /// writer.set_patient_info("P001", "M", "01-JAN-1990", "Test")?;
    /// 
    /// // Initially no annotations
    /// assert_eq!(writer.annotation_count(), 0);
    /// 
    /// writer.add_annotation(1.0, None, "Event 1")?;
    /// assert_eq!(writer.annotation_count(), 1);
    /// 
    /// writer.add_annotation(2.0, Some(0.5), "Event 2")?;
    /// assert_eq!(writer.annotation_count(), 2);
    /// 
    /// # // Cleanup
    /// # std::fs::remove_file("count_test.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    pub fn annotation_count(&self) -> usize {
        self.annotations.len()
    }

    /// Generates TAL data for a specific annotation channel (遵循edflib多通道设计)
    /// 
    /// 这个方法实现了edflib.c中的多注释通道TAL数据分发策略。
    /// 注释数据会按照通道索引分布到不同的注释通道中，遵循EDF+标准。
    /// 
    /// # Arguments
    /// 
    /// * `data_record_index` - 数据记录索引
    /// * `channel_idx` - 注释通道索引 (0 到 nr_annot_chns-1)
    /// 
    /// # Channel Distribution Strategy
    /// 
    /// 遵循edflib.c的设计：
    /// - 第一个注释通道 (0): 包含时间戳 + 按顺序分配的注释
    /// - 其他注释通道: 循环分配剩余注释
    /// 
    /// # Returns
    /// 
    /// 返回120字节的TAL数据，严格符合EDF+标准格式
    fn generate_annotation_tal_for_channel(&self, data_record_index: usize, channel_idx: usize) -> Result<Vec<u8>> {
        let mut tal_data = Vec::with_capacity(EDFLIB_ANNOTATION_BYTES);
        
        // 数据记录的时间范围
        let data_record_time_start = data_record_index as f64 * (self.datarecord_duration as f64 / EDFLIB_TIME_DIMENSION as f64);
        let data_record_time_end = (data_record_index + 1) as f64 * (self.datarecord_duration as f64 / EDFLIB_TIME_DIMENSION as f64);
        
        // 第一个注释通道处理时间戳记录（遵循edflib设计）
        if channel_idx == 0 {
            // 时间戳注释，格式: "+<onset>\x14\x14\x00"
            tal_data.push(b'+');
            
            // 添加子秒精度支持
            if data_record_index == 0 && self.starttime_subsecond > 0 {
                // 第一个记录包含子秒开始时间
                let time_with_subsecond = data_record_time_start + (self.starttime_subsecond as f64 / EDFLIB_TIME_DIMENSION as f64);
                let time_str = format!("{:.7}", time_with_subsecond).trim_end_matches('0').trim_end_matches('.').to_string();
                tal_data.extend_from_slice(time_str.as_bytes());
            } else {
                let time_str = if data_record_time_start.fract() == 0.0 {
                    format!("{}", data_record_time_start as i64)
                } else {
                    format!("{:.6}", data_record_time_start).trim_end_matches('0').trim_end_matches('.').to_string()
                };
                tal_data.extend_from_slice(time_str.as_bytes());
            }
            
            tal_data.push(0x14); // ASCII 20 - start of annotation
            tal_data.push(0x14); // ASCII 20 - end of annotation (empty)
            tal_data.push(0x00); // Null terminator
        }
        
        // 查找属于当前数据记录和注释通道的注释
        let mut record_annotations = Vec::new();
        for (annot_idx, annotation) in self.annotations.iter().enumerate() {
            let annotation_time = annotation.onset as f64 / EDFLIB_TIME_DIMENSION as f64;
            
            // 检查注释是否属于当前数据记录
            if annotation_time >= data_record_time_start && annotation_time < data_record_time_end {
                // 按照edflib策略分配注释到通道
                let target_channel = if self.nr_annot_chns == 1 {
                    0 // 单通道模式，所有注释都在通道0
                } else {
                    // 多通道模式：循环分配
                    // 第一个注释通道处理时间戳，其他注释按索引分配
                    if channel_idx == 0 {
                        // 第一个通道处理部分注释（时间戳优先）
                        if annot_idx % self.nr_annot_chns == 0 {
                            0
                        } else {
                            continue; // 不属于通道0的注释跳过
                        }
                    } else {
                        // 其他通道按循环分配
                        if annot_idx % self.nr_annot_chns == channel_idx {
                            channel_idx
                        } else {
                            continue; // 不属于当前通道的注释跳过
                        }
                    }
                };
                
                if target_channel == channel_idx {
                    record_annotations.push(annotation);
                }
            }
        }
        
        // 添加分配给当前通道的注释
        for annotation in record_annotations {
            let annotation_time = annotation.onset as f64 / EDFLIB_TIME_DIMENSION as f64;
            
            // 计算基本注释结构所需的最小空间
            let time_str = format!("{:.7}", annotation_time).trim_end_matches('0').trim_end_matches('.').to_string();
            let mut min_needed_space = 1 + time_str.len() + 2 + 1; // +, time, \x14, \x14 (不包括描述)
            
            if annotation.duration >= 0 {
                let duration_str = format!("{:.7}", annotation.duration as f64 / EDFLIB_TIME_DIMENSION as f64)
                    .trim_end_matches('0').trim_end_matches('.').to_string();
                min_needed_space += 1 + duration_str.len(); // \x15 + duration
            }
            
            // 检查是否有足够的最小空间来容纳注释结构（描述可以被截断）
            if tal_data.len() + min_needed_space > EDFLIB_ANNOTATION_BYTES - 2 {
                break; // 没有足够空间放入基本结构，跳过剩余注释
            }
            
            // 格式: "+<onset>[\x15<duration>]\x14<description>\x14"
            tal_data.push(b'+');
            tal_data.extend_from_slice(time_str.as_bytes());
            
            // 添加持续时间（如果指定）
            if annotation.duration >= 0 {
                tal_data.push(0x15); // ASCII 21 - duration separator
                let duration_str = format!("{:.7}", annotation.duration as f64 / EDFLIB_TIME_DIMENSION as f64)
                    .trim_end_matches('0').trim_end_matches('.').to_string();
                tal_data.extend_from_slice(duration_str.as_bytes());
            }
            
            tal_data.push(0x14); // ASCII 20 - start of description
            
            // 截断过长的描述（遵循edflib限制）
            let description_bytes = annotation.description.as_bytes();
            let max_desc_len = EDFLIB_WRITE_MAX_ANNOTATION_LEN.min(
                EDFLIB_ANNOTATION_BYTES - tal_data.len() - 2 // 为结束符预留空间
            );
            let desc_len = description_bytes.len().min(max_desc_len);
            tal_data.extend_from_slice(&description_bytes[..desc_len]);
            
            tal_data.push(0x14); // ASCII 20 - end of annotation
        }
        
        // 填充到确切的120字节，用零填充（遵循edflib）
        tal_data.resize(EDFLIB_ANNOTATION_BYTES, 0x00);
        Ok(tal_data)
    }

    // 添加subsecond开始时间支持
    pub fn set_subsecond_starttime(&mut self, subsecond: i64) -> Result<()> {
        if self.header_written {
            return Err(EdfError::InvalidFormat("Cannot modify subsecond start time after writing header".to_string()));
        }
        
        if subsecond < 0 || subsecond >= EDFLIB_TIME_DIMENSION {
            return Err(EdfError::InvalidFormat("Subsecond must be between 0 and 9999999".to_string()));
        }
        
        self.starttime_subsecond = subsecond;
        Ok(())
    }

    /// 写入信号头部，注释信号始终放在末尾 (简化edflib设计)
    fn write_signal_headers_with_annotations(&mut self, annotation_signals: &[SignalParam]) -> Result<()> {
        // 构建最终的信号列表，注释信号始终在末尾
        let mut all_signals = Vec::new();
        all_signals.extend_from_slice(&self.signals);
        all_signals.extend_from_slice(annotation_signals);
        
        // 按照edflib的字段顺序写入，每个字段所有信号一起写
        
        // 1. 标签 (16字节 × 信号数)
        for signal in &all_signals {
            let mut field_data = [b' '; 16];
            let label_bytes = signal.label.as_bytes();
            let len = label_bytes.len().min(16);
            field_data[..len].copy_from_slice(&label_bytes[..len]);
            self.file.write_all(&field_data)?;
        }
        
        // 2. 传感器 (80字节 × 信号数)
        for signal in &all_signals {
            let mut field_data = [b' '; 80];
            let trans_bytes = signal.transducer.as_bytes();
            let len = trans_bytes.len().min(80);
            field_data[..len].copy_from_slice(&trans_bytes[..len]);
            self.file.write_all(&field_data)?;
        }
        
        // 3. 物理单位 (8字节 × 信号数)
        for signal in &all_signals {
            let mut field_data = [b' '; 8];
            let unit_bytes = signal.physical_dimension.as_bytes();
            let len = unit_bytes.len().min(8);
            field_data[..len].copy_from_slice(&unit_bytes[..len]);
            self.file.write_all(&field_data)?;
        }
        
        // 4. 物理最小值 (8字节 × 信号数)
        for signal in &all_signals {
            let phys_min_str = format!("{:<8}", signal.physical_min);
            self.file.write_all(phys_min_str.as_bytes())?;
        }
        
        // 5. 物理最大值 (8字节 × 信号数)
        for signal in &all_signals {
            let phys_max_str = format!("{:<8}", signal.physical_max);
            self.file.write_all(phys_max_str.as_bytes())?;
        }
        
        // 6. 数字最小值 (8字节 × 信号数)
        for signal in &all_signals {
            let dig_min_str = format!("{:<8}", signal.digital_min);
            self.file.write_all(dig_min_str.as_bytes())?;
        }
        
        // 7. 数字最大值 (8字节 × 信号数)
        for signal in &all_signals {
            let dig_max_str = format!("{:<8}", signal.digital_max);
            self.file.write_all(dig_max_str.as_bytes())?;
        }
        
        // 8. 预滤波 (80字节 × 信号数)
        for signal in &all_signals {
            let mut field_data = [b' '; 80];
            let prefilter_bytes = signal.prefilter.as_bytes();
            let len = prefilter_bytes.len().min(80);
            field_data[..len].copy_from_slice(&prefilter_bytes[..len]);
            self.file.write_all(&field_data)?;
        }
        
        // 9. 每记录样本数 (8字节 × 信号数)
        for signal in &all_signals {
            let samples_str = format!("{:<8}", signal.samples_per_record);
            self.file.write_all(samples_str.as_bytes())?;
        }
        
        // 10. 保留字段 (32字节 × 信号数)
        for _signal in &all_signals {
            let field_data = [b' '; 32];
            self.file.write_all(&field_data)?;
        }
        
        Ok(())
    }

    /// Sets the number of annotation signals (channels)
    /// 
    /// EDF+ supports multiple annotation signals according to the standard.
    /// This follows the edflib design where you can have 1-64 annotation channels.
    /// 
    /// # Arguments
    /// 
    /// * `annot_signals` - Number of annotation signals (1-64)
    /// 
    /// # Errors
    /// 
    /// * `EdfError::InvalidFormat` - Trying to modify after header written
    /// * `EdfError::InvalidArgument` - Invalid number of annotation signals
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::EdfWriter;
    /// 
    /// let mut writer = EdfWriter::create("multi_annot.edf")?;
    /// 
    /// // Set 3 annotation channels for complex event coding
    /// writer.set_number_of_annotation_signals(3)?;
    /// 
    /// # // Cleanup (hidden from docs)
    /// # std::fs::remove_file("multi_annot.edf").ok();
    /// # Ok::<(), edfplus::EdfError>(())
    /// ```
    pub fn set_number_of_annotation_signals(&mut self, annot_signals: usize) -> Result<()> {
        if self.header_written {
            return Err(EdfError::InvalidFormat("Cannot modify annotation signals after writing header".to_string()));
        }
        
        if annot_signals == 0 || annot_signals > EDFLIB_MAX_ANNOTATION_CHANNELS {
            return Err(EdfError::InvalidFormat(format!(
                "Annotation signals must be 1-{}, got {}",
                EDFLIB_MAX_ANNOTATION_CHANNELS, annot_signals
            )));
        }
        
        self.nr_annot_chns = annot_signals;
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    
    fn create_test_signal() -> SignalParam {
        SignalParam {
            label: "Test EEG".to_string(),
            samples_in_file: 0,
            physical_max: 100.0,
            physical_min: -100.0,
            digital_max: 32767,
            digital_min: -32768,
            samples_per_record: 256,
            physical_dimension: "uV".to_string(),
            prefilter: "HP:0.1Hz LP:70Hz".to_string(),
            transducer: "AgAgCl electrodes".to_string(),
        }
    }
    
    fn cleanup_test_file(filename: &str) {
        if Path::new(filename).exists() {
            fs::remove_file(filename).ok();
        }
    }

    #[test]
    fn test_edf_writer_default_annotation_settings() {
        let writer = EdfWriter::create("test_default.edf").unwrap();
        assert_eq!(writer.nr_annot_chns, 1);
        cleanup_test_file("test_default.edf");
    }

    #[test]
    fn test_set_number_of_annotation_signals() {
        let mut writer = EdfWriter::create("test_annot_num.edf").unwrap();
        
        // Test valid ranges
        assert!(writer.set_number_of_annotation_signals(1).is_ok());
        assert_eq!(writer.nr_annot_chns, 1);
        
        assert!(writer.set_number_of_annotation_signals(32).is_ok());
        assert_eq!(writer.nr_annot_chns, 32);
        
        assert!(writer.set_number_of_annotation_signals(64).is_ok());
        assert_eq!(writer.nr_annot_chns, 64);
        
        // Test invalid ranges
        assert!(writer.set_number_of_annotation_signals(0).is_err());
        assert!(writer.set_number_of_annotation_signals(65).is_err());
        
        cleanup_test_file("test_annot_num.edf");
    }

    #[test]
    fn test_modification_after_header_written() {
        let mut writer = EdfWriter::create("test_locked.edf").unwrap();
        writer.set_patient_info("P001", "M", "01-JAN-1990", "Test").unwrap();
        writer.add_signal(create_test_signal()).unwrap();
        
        // Write some data to trigger header writing
        let samples = vec![10.0; 256];
        writer.write_samples(&[samples]).unwrap();
        
        // Now modifications should fail
        assert!(writer.set_number_of_annotation_signals(3).is_err());
        
        cleanup_test_file("test_locked.edf");
    }

    #[test]
    fn test_multi_channel_annotation_header_creation() {
        let mut writer = EdfWriter::create("test_multi_header.edf").unwrap();
        writer.set_patient_info("P001", "M", "01-JAN-1990", "Test").unwrap();
        writer.set_number_of_annotation_signals(3).unwrap();
        
        // Add regular signals
        writer.add_signal(create_test_signal()).unwrap();
        
        // Write data to trigger header creation
        let samples = vec![10.0; 256];
        writer.write_samples(&[samples]).unwrap();
        writer.finalize().unwrap();
        
        // Verify file was created
        assert!(Path::new("test_multi_header.edf").exists());
        
        cleanup_test_file("test_multi_header.edf");
    }

    #[test]
    fn test_annotation_tal_generation() {
        let mut writer = EdfWriter::create("test_tal.edf").unwrap();
        writer.set_patient_info("P001", "M", "01-JAN-1990", "Test").unwrap();
        writer.set_number_of_annotation_signals(2).unwrap();
        
        // Add a test annotation
        writer.add_annotation(0.0, None, "Test Event").unwrap();
        writer.add_annotation(1.5, Some(2.0), "Another Event").unwrap();
        
        writer.add_signal(create_test_signal()).unwrap();
        
        // Test TAL generation for multiple channels
        let tal_0 = writer.generate_annotation_tal_for_channel(0, 0).unwrap();
        let tal_1 = writer.generate_annotation_tal_for_channel(1, 0).unwrap();
        
        // Both channels should have some content
        assert!(!tal_0.is_empty());
        assert!(!tal_1.is_empty());
        
        // TAL should contain the time annotations (checking as bytes)
        let tal_0_str = String::from_utf8_lossy(&tal_0);
        let tal_1_str = String::from_utf8_lossy(&tal_1);
        assert!(tal_0_str.contains("+0\x14") || tal_1_str.contains("+0\x14"));
        
        cleanup_test_file("test_tal.edf");
    }

    #[test]
    fn test_complete_multi_channel_workflow() {
        let filename = "test_complete_workflow.edf";
        
        // Create writer with multiple annotation channels
        let mut writer = EdfWriter::create(filename).unwrap();
        writer.set_patient_info("P001", "M", "01-JAN-1990", "Multi-channel Test").unwrap();
        writer.set_number_of_annotation_signals(3).unwrap();
        
        // Add multiple signals
        for i in 0..4 {
            let mut signal = create_test_signal();
            signal.label = format!("EEG_Ch{}", i + 1);
            signal.physical_max = 200.0;
            signal.physical_min = -200.0;
            writer.add_signal(signal).unwrap();
        }
        
        // Add annotations
        writer.add_annotation(0.0, None, "Recording Start").unwrap();
        writer.add_annotation(2.5, Some(1.0), "Artifact").unwrap();
        writer.add_annotation(5.0, None, "Eyes Closed").unwrap();
        writer.add_annotation(10.0, None, "Eyes Open").unwrap();
        
        // Write 10 seconds of test data
        for second in 0..10 {
            let mut all_samples = Vec::new();
            
            for ch in 0..4 {
                let mut channel_samples = Vec::new();
                for sample in 0..256 {
                    let t = (second * 256 + sample) as f64 / 256.0;
                    // Generate different frequencies for each channel
                    let freq = 10.0 + ch as f64 * 2.0; // 10Hz, 12Hz, 14Hz, 16Hz
                    let value = 50.0 * (2.0 * std::f64::consts::PI * freq * t).sin();
                    channel_samples.push(value);
                }
                all_samples.push(channel_samples);
            }
            
            writer.write_samples(&all_samples).unwrap();
        }
        
        writer.finalize().unwrap();
        
        // Verify file exists and has reasonable size
        let metadata = fs::metadata(filename).unwrap();
        assert!(metadata.len() > 0);
        println!("Created multi-channel EDF+ file: {} bytes", metadata.len());
        
        cleanup_test_file(filename);
    }
}
