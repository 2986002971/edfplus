use chrono::{NaiveDate, NaiveTime};

/// Supported EDF file types
/// 
/// Currently only EDF+ format is supported as it's the modern standard
/// with support for annotations and extended metadata.
/// 
/// This type is used internally for file format validation.
#[derive(Debug, Clone, PartialEq)]
pub enum FileType {
    /// EDF+ format - European Data Format Plus
    /// 
    /// This is the recommended format for new recordings as it supports:
    /// - Annotations and events
    /// - Extended patient information  
    /// - Equipment information
    /// - Standardized field formats
    EdfPlus,
}

/// Signal parameters and metadata
/// 
/// Contains all the information needed to describe a signal in an EDF+ file,
/// including physical and digital value ranges, labels, and conversion parameters.
#[derive(Debug, Clone)]
pub struct SignalParam {
    /// Signal label/name (e.g., "EEG Fp1", "ECG Lead II")
    /// 
    /// # Examples
    /// 
    /// Common signal labels:
    /// - EEG signals: "EEG Fp1", "EEG Fp2", "EEG C3", "EEG C4"
    /// - ECG signals: "ECG Lead I", "ECG Lead II", "ECG Lead V1"
    /// - EMG signals: "EMG Left Deltoid", "EMG Right Biceps"
    pub label: String,
    
    /// Total number of samples for this signal in the file
    pub samples_in_file: i64,
    
    /// Maximum physical value (e.g., +200.0 µV)
    /// 
    /// This represents the highest real-world measurement value
    /// that corresponds to the digital maximum value.
    pub physical_max: f64,
    
    /// Minimum physical value (e.g., -200.0 µV) 
    /// 
    /// This represents the lowest real-world measurement value
    /// that corresponds to the digital minimum value.
    pub physical_min: f64,
    
    /// Maximum digital value (typically 32767 for EDF+)
    /// 
    /// This is the highest integer value that can be stored
    /// in the file for this signal.
    pub digital_max: i32,
    
    /// Minimum digital value (typically -32768 for EDF+)
    /// 
    /// This is the lowest integer value that can be stored
    /// in the file for this signal.
    pub digital_min: i32,
    
    /// Number of samples per data record
    /// 
    /// For a 1-second data record, this equals the sampling frequency.
    /// For example, 256 samples per record = 256 Hz sampling rate.
    pub samples_per_record: i32,
    
    /// Physical dimension/unit (e.g., "µV", "mV", "BPM")
    /// 
    /// # Examples
    /// 
    /// Common units:
    /// - "uV" or "µV" for EEG signals
    /// - "mV" for ECG signals  
    /// - "BPM" for heart rate
    /// - "%" for oxygen saturation
    /// - "mmHg" for blood pressure
    pub physical_dimension: String,
    
    /// Prefilter information (e.g., "HP:0.1Hz LP:70Hz")
    /// 
    /// Describes any analog or digital filtering applied to the signal
    /// before digitization or storage.
    pub prefilter: String,
    
    /// Transducer type (e.g., "AgAgCl cup electrodes")
    /// 
    /// Describes the sensor or electrode used to acquire the signal.
    pub transducer: String,
}

impl SignalParam {
    /// Calculate the bit value (resolution) for this signal
    /// 
    /// This determines how much each digital unit represents in physical units.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::SignalParam;
    /// 
    /// let signal = SignalParam {
    ///     label: "Test".to_string(),
    ///     samples_in_file: 1000,
    ///     physical_max: 100.0,
    ///     physical_min: -100.0,
    ///     digital_max: 32767,
    ///     digital_min: -32768,
    ///     samples_per_record: 256,
    ///     physical_dimension: "uV".to_string(),
    ///     prefilter: "".to_string(),
    ///     transducer: "".to_string(),
    /// };
    /// 
    /// let bit_value = signal.bit_value();
    /// // For a ±100µV range over ±32767 digital range:
    /// // bit_value = 200.0 / 65535 ≈ 0.00305 µV per bit
    /// assert!((bit_value - 0.00305).abs() < 0.0001);
    /// ```
    pub fn bit_value(&self) -> f64 {
        (self.physical_max - self.physical_min) / 
        (self.digital_max - self.digital_min) as f64
    }
    
    /// Calculate the offset for digital to physical conversion
    /// 
    /// This is used internally for the conversion calculations.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::SignalParam;
    /// 
    /// let signal = SignalParam {
    ///     label: "Test".to_string(),
    ///     samples_in_file: 1000,
    ///     physical_max: 200.0,
    ///     physical_min: -200.0,
    ///     digital_max: 32767,
    ///     digital_min: -32768,
    ///     samples_per_record: 256,
    ///     physical_dimension: "uV".to_string(),
    ///     prefilter: "".to_string(),
    ///     transducer: "".to_string(),
    /// };
    /// 
    /// let offset = signal.offset();
    /// // The offset should position the conversion correctly  
    /// assert!(offset.abs() < 1.0); // Should be close to zero for symmetric ranges
    /// ```
    pub fn offset(&self) -> f64 {
        self.physical_max / self.bit_value() - self.digital_max as f64
    }
    
    /// Convert a digital value to its corresponding physical value
    /// 
    /// # Arguments
    /// 
    /// * `digital_value` - The digital value from the EDF file (typically -32768 to 32767)
    /// 
    /// # Returns
    /// 
    /// The corresponding physical measurement value with proper units
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::SignalParam;
    /// 
    /// let signal = SignalParam {
    ///     label: "EEG Fp1".to_string(),
    ///     samples_in_file: 1000,
    ///     physical_max: 200.0,   // +200 µV
    ///     physical_min: -200.0,  // -200 µV
    ///     digital_max: 32767,
    ///     digital_min: -32768,
    ///     samples_per_record: 256,
    ///     physical_dimension: "uV".to_string(),
    ///     prefilter: "".to_string(),
    ///     transducer: "".to_string(),
    /// };
    /// 
    /// // Test maximum value
    /// let max_physical = signal.to_physical(32767);
    /// assert!((max_physical - 200.0).abs() < 0.1);
    /// 
    /// // Test minimum value  
    /// let min_physical = signal.to_physical(-32768);
    /// assert!((min_physical - (-200.0)).abs() < 0.1);
    /// 
    /// // Test zero (should be near middle of range)
    /// let zero_physical = signal.to_physical(0);
    /// assert!(zero_physical.abs() < 1.0);
    /// 
    /// // Test half-scale positive
    /// let half_physical = signal.to_physical(16384);
    /// assert!((half_physical - 100.0).abs() < 1.0);
    /// ```
    pub fn to_physical(&self, digital_value: i32) -> f64 {
        self.bit_value() * (self.offset() + digital_value as f64)
    }
    
    /// Convert a physical value to its corresponding digital value
    /// 
    /// # Arguments
    /// 
    /// * `physical_value` - The real-world measurement value
    /// 
    /// # Returns
    /// 
    /// The corresponding digital value that should be stored in the EDF file
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::SignalParam;
    /// 
    /// let signal = SignalParam {
    ///     label: "ECG Lead II".to_string(),
    ///     samples_in_file: 1000,
    ///     physical_max: 5.0,     // +5 mV
    ///     physical_min: -5.0,    // -5 mV
    ///     digital_max: 32767,
    ///     digital_min: -32768,
    ///     samples_per_record: 256,
    ///     physical_dimension: "mV".to_string(),
    ///     prefilter: "".to_string(),
    ///     transducer: "".to_string(),
    /// };
    /// 
    /// // Test maximum value
    /// let max_digital = signal.to_digital(5.0);
    /// assert!((max_digital - 32767).abs() <= 1);
    /// 
    /// // Test minimum value
    /// let min_digital = signal.to_digital(-5.0);
    /// assert!((min_digital - (-32768)).abs() <= 1);
    /// 
    /// // Test zero
    /// let zero_digital = signal.to_digital(0.0);
    /// assert!(zero_digital.abs() <= 1);
    /// 
    /// // Test positive value
    /// let pos_digital = signal.to_digital(2.5);
    /// assert!((pos_digital - 16384).abs() <= 100);
    /// ```
    pub fn to_digital(&self, physical_value: f64) -> i32 {
        let digital = (physical_value / self.bit_value()) - self.offset();
        digital.round() as i32
    }
}

/// Annotation or event marker in an EDF+ file
/// 
/// Annotations are used to mark events, artifacts, or other points of interest
/// in the recording timeline.
/// 
/// # Examples
/// 
/// ```rust
/// use edfplus::Annotation;
/// 
/// // Create an annotation for a seizure event
/// let seizure_event = Annotation {
///     onset: 1500000000,  // 150 seconds after start (in 100ns units)
///     duration: 300000000, // 30 seconds duration (in 100ns units)  
///     description: "Seizure detected".to_string(),
/// };
/// 
/// // Convert onset to seconds
/// let onset_seconds = seizure_event.onset as f64 / 10_000_000.0;
/// assert_eq!(onset_seconds, 150.0);
/// 
/// // Convert duration to seconds
/// let duration_seconds = seizure_event.duration as f64 / 10_000_000.0;
/// assert_eq!(duration_seconds, 30.0);
/// ```
#[derive(Debug, Clone)]
pub struct Annotation {
    /// Onset time in 100-nanosecond units since recording start
    /// 
    /// To convert to seconds: `onset as f64 / 10_000_000.0`
    pub onset: i64,
    
    /// Duration in 100-nanosecond units (-1 if unknown/instantaneous)
    /// 
    /// To convert to seconds: `duration as f64 / 10_000_000.0`
    pub duration: i64,
    
    /// UTF-8 description of the event
    /// 
    /// Common annotation types:
    /// - "Sleep stage 1", "Sleep stage 2", "Sleep stage 3", "Sleep stage 4", "Sleep stage REM"
    /// - "Seizure", "Spike", "Sharp wave"
    /// - "Movement artifact", "Eye blink", "Muscle artifact"  
    /// - "Stimulus onset", "Response", "Button press"
    pub description: String,
}

/// Complete EDF+ file header information
/// 
/// Contains all metadata about the recording, including patient information,
/// recording parameters, and signal definitions.
/// 
/// # Examples
/// 
/// ```rust
/// use edfplus::{EdfReader, EdfWriter, SignalParam};
/// # use std::fs;
/// 
/// # // Create a test file first
/// # let mut writer = EdfWriter::create("test_header_example.edf").unwrap();
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
/// let mut reader = EdfReader::open("test_header_example.edf").unwrap();
/// let header = reader.header();
/// 
/// println!("Recording duration: {:.2} seconds", 
///     header.file_duration as f64 / 10_000_000.0);
/// println!("Number of signals: {}", header.signals.len());
/// println!("Patient: {} ({})", header.patient_name, header.patient_code);
/// println!("Equipment: {}", header.equipment);
/// 
/// for (i, signal) in header.signals.iter().enumerate() {
///     println!("Signal {}: {} ({} {})", 
///         i, signal.label, signal.physical_dimension, 
///         signal.samples_per_record);
/// }
/// 
/// # // Cleanup
/// # drop(reader);
/// # fs::remove_file("test_header_example.edf").ok();
/// ```
pub struct EdfHeader {
    /// List of all signals in the file (excluding annotation signals)
    /// 
    /// Each signal contains its own parameters like sampling rate,
    /// physical ranges, labels, etc.
    pub signals: Vec<SignalParam>,
    
    /// Total duration of the recording in 100-nanosecond units
    /// 
    /// To convert to seconds: `file_duration as f64 / 10_000_000.0`
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use edfplus::{EdfReader, EdfWriter, SignalParam};
    /// # use std::fs;
    /// 
    /// # // Create a test file first
    /// # let mut writer = EdfWriter::create("test_duration.edf").unwrap();
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
    /// let mut reader = EdfReader::open("test_duration.edf").unwrap();
    /// let header = reader.header();
    /// 
    /// let duration_seconds = header.file_duration as f64 / 10_000_000.0;
    /// let duration_minutes = duration_seconds / 60.0;
    /// println!("Recording length: {:.1} minutes", duration_minutes);
    /// 
    /// # // Cleanup
    /// # drop(reader);
    /// # fs::remove_file("test_duration.edf").ok();
    /// ```
    pub file_duration: i64,
    
    /// Recording start date
    pub start_date: NaiveDate,
    
    /// Recording start time  
    pub start_time: NaiveTime,
    
    /// Subsecond precision for start time (100-nanosecond units)
    pub starttime_subsecond: i64,
    
    /// Number of data records in the file
    /// 
    /// Each data record typically represents 1 second of data,
    /// but can be configured differently.
    pub datarecords_in_file: i64,
    
    /// Duration of each data record in 100-nanosecond units
    /// 
    /// Default is 10,000,000 (1 second). Shorter records provide
    /// better temporal resolution for annotations.
    pub datarecord_duration: i64,
    
    /// Total number of annotations/events in the file
    pub annotations_in_file: i64,
    
    // EDF+ specific patient information fields
    
    /// Patient identification code
    /// 
    /// Should be unique identifier, often anonymized for privacy.
    /// Example: "MCH-0234567" or "ANON-001"
    pub patient_code: String,
    
    /// Patient sex/gender
    /// 
    /// Standard values: "M" (male), "F" (female), "X" (unknown)
    pub sex: String,
    
    /// Patient birth date in DD-MMM-YYYY format
    /// 
    /// Example: "02-MAY-1951" or "X" if unknown/anonymized
    pub birthdate: String,
    
    /// Patient name
    /// 
    /// Often anonymized as "X" for privacy protection
    pub patient_name: String,
    
    /// Additional patient information
    /// 
    /// Free text field for additional patient details
    pub patient_additional: String,
    
    // EDF+ specific recording information fields
    
    /// Administration code or hospital department
    /// 
    /// Example: "PSG-LAB" or "NEURO-ICU"  
    pub admin_code: String,
    
    /// Technician name or code
    /// 
    /// Person responsible for the recording
    pub technician: String,
    
    /// Recording equipment description
    /// 
    /// Brand and model of the recording system
    /// Example: "Nihon Kohden EEG-1200" or "Grass Telefactor"
    pub equipment: String,
    
    /// Additional recording information
    /// 
    /// Free text field for recording details, protocols, etc.
    pub recording_additional: String,
}
