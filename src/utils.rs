use crate::error::{EdfError, Result};

/// 检查字符串是否为有效的整数
pub fn is_integer_number(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    
    // 简单的整数检查
    s.chars().next().map_or(false, |first| first == '+' || first == '-' || first.is_ascii_digit()) &&
    s.chars().skip(if s.starts_with('+') || s.starts_with('-') { 1 } else { 0 })
        .all(|c| c.is_ascii_digit() || c == ' ') &&
    s.chars().any(|c| c.is_ascii_digit())
}

/// 检查字符串是否为有效的数字（包括浮点数）
pub fn is_number(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    
    // 使用Rust的parse来验证
    s.parse::<f64>().is_ok()
}

/// 解析EDF时间字符串为100纳秒单位
pub fn parse_edf_time(s: &str) -> Result<i64> {
    let s = s.trim();
    
    if s.is_empty() {
        return Err(EdfError::InvalidFormat("Empty time string".to_string()));
    }
    
    // 处理符号
    let (negative, s) = if s.starts_with('-') {
        (true, &s[1..])
    } else if s.starts_with('+') {
        (false, &s[1..])
    } else {
        (false, s)
    };
    
    let mut value = 0i64;
    
    if let Some(dot_pos) = s.find('.') {
        // 有小数部分
        let integer_part = &s[..dot_pos];
        let decimal_part = &s[dot_pos + 1..];
        
        // 解析整数部分
        if !integer_part.is_empty() {
            value += integer_part.parse::<i64>()
                .map_err(|_| EdfError::InvalidFormat("Invalid integer part".to_string()))?
                * crate::EDFLIB_TIME_DIMENSION;
        }
        
        // 解析小数部分（最多7位精度）
        if !decimal_part.is_empty() {
            let decimal_str = if decimal_part.len() > 7 {
                &decimal_part[..7]
            } else {
                decimal_part
            };
            
            let decimal_value = decimal_str.parse::<i64>()
                .map_err(|_| EdfError::InvalidFormat("Invalid decimal part".to_string()))?;
            
            let scale = 10i64.pow(7 - decimal_str.len() as u32);
            value += decimal_value * scale;
        }
    } else {
        // 只有整数部分
        value = s.parse::<i64>()
            .map_err(|_| EdfError::InvalidFormat("Invalid integer".to_string()))?
            * crate::EDFLIB_TIME_DIMENSION;
    }
    
    if negative {
        value = -value;
    }
    
    Ok(value)
}

/// 移除字符串前后的空格
pub fn trim_padding_spaces(s: &mut String) {
    let trimmed = s.trim().to_string();
    s.clear();
    s.push_str(&trimmed);
}

/// 非本地化的整数解析（避免受系统locale影响）
pub fn atoi_nonlocalized(s: &str) -> i32 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    
    s.parse().unwrap_or(0)
}

/// 非本地化的浮点数解析
pub fn atof_nonlocalized(s: &str) -> f64 {
    let s = s.trim();
    if s.is_empty() {
        return 0.0;
    }
    
    s.parse().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_integer_number() {
        assert!(is_integer_number("123"));
        assert!(is_integer_number("-456"));
        assert!(is_integer_number("+789"));
        assert!(is_integer_number("0"));
        assert!(!is_integer_number("12.34"));
        assert!(!is_integer_number("abc"));
        assert!(!is_integer_number(""));
    }

    #[test]
    fn test_parse_edf_time() {
        assert_eq!(parse_edf_time("1").unwrap(), 10_000_000);
        assert_eq!(parse_edf_time("1.5").unwrap(), 15_000_000);
        assert_eq!(parse_edf_time("-2.5").unwrap(), -25_000_000);
        assert_eq!(parse_edf_time("+0.0000001").unwrap(), 1);
    }
}
