use obd_equation_rs::ExpressionEvaluator;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create evaluator
    let mut evaluator = ExpressionEvaluator::new()?;

    // Simple arithmetic
    let result = evaluator.evaluate("2 + 3", &HashMap::new())?;
    println!("2 + 3 = {}", result);
    assert_eq!(result, 5.0);

    // With variables
    let mut variables = HashMap::new();
    variables.insert("x".to_string(), 10.0);
    variables.insert("y".to_string(), 5.0);

    let result = evaluator.evaluate("x * 2 + y", &variables)?;
    println!("x * 2 + y = {} (with x=10, y=5)", result);
    assert_eq!(result, 25.0);

    println!("Basic evaluation tests passed!");
    Ok(())
}
