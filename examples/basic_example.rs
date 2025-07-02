use edfplus::{EdfReader, Result};

fn main() -> Result<()> {
    println!("EDF+ Library Example");
    println!("Library Version: {}", edfplus::version());
    
    // 尝试读取一个EDF文件（如果存在的话）
    let test_files = [
        "test_data/test_generated.edf",
    ];
    
    for file_path in &test_files {
        match EdfReader::open(file_path) {
            Ok(reader) => {
                println!("Successfully opened: {}", file_path);
                let header = reader.header();
                println!("File format: EDF+ (European Data Format Plus)");
                println!("Number of signals: {}", header.signals.len());
                println!("File duration: {} (100ns units)", header.file_duration);
                println!("Data records: {}", header.datarecords_in_file);
                
                for (i, signal) in header.signals.iter().enumerate() {
                    println!("Signal {}: {} ({} {})", 
                        i, signal.label, signal.physical_dimension,
                        signal.samples_in_file);
                }
                return Ok(());
            }
            Err(e) => {
                println!("Could not open {}: {}", file_path, e);
            }
        }
    }
    
    println!("No test EDF files found. The library is ready to use!");
    println!("To test with a real file, place an EDF+ file in the current directory and run again.");
    
    Ok(())
}
