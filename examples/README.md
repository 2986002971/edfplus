# EDF+ Reader Example

This example demonstrates basic usage of the EDF+ library.

## Basic Usage

```rust
use edfplus::{EdfReader, Result};

fn main() -> Result<()> {
    // Open an EDF+ file
    let mut reader = EdfReader::open("example.edf")?;
    
    // Get header information
    let header = reader.header();
    println!("Signals: {}", header.signals.len());
    
    // Read physical samples from signal 0
    let samples = reader.read_physical_samples(0, 1000)?;
    println!("Read {} samples", samples.len());
    
    Ok(())
}
```

## Features Implemented

- ✅ Reading EDF+ files
- ✅ Signal parameter extraction
- ✅ Physical/digital value conversion
- ✅ Sample position tracking
- ✅ Error handling

## TODO

- [ ] Annotation reading
- [ ] File writing functionality
- [ ] Advanced signal processing
