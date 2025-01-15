use std::collections::{HashMap, HashSet};
use std::{error, fmt};

#[derive(Debug, Clone)]
pub struct MissingDependencyError {
    pub(crate) module_name: String,
    pub(crate) dependency_name: String,
}

impl fmt::Display for MissingDependencyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Missing dependency '{}' for module '{}'",
            self.dependency_name, self.module_name
        )
    }
}

impl error::Error for MissingDependencyError {}

#[derive(Debug, Clone)]
pub struct CircularDependencyError {
    pub(crate) module_name: String,
}

impl fmt::Display for CircularDependencyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Circular dependency detected for module '{}'",
            self.module_name
        )
    }
}

impl error::Error for CircularDependencyError {}

pub fn sort_dependencies<'a>(
    dependencies: &HashMap<&'a str, Vec<&'a str>>,
) -> Result<Vec<&'a str>, Box<dyn error::Error>> {
    let mut sorted = Vec::new();
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();

    for &module in dependencies.keys() {
        if !visited.contains(module) {
            visit(
                module,
                dependencies,
                &mut sorted,
                &mut visited,
                &mut visiting,
            )?;
        }
    }

    Ok(sorted)
}

fn visit<'a>(
    module: &'a str,
    dependencies: &HashMap<&str, Vec<&'a str>>,
    sorted: &mut Vec<&'a str>,
    visited: &mut HashSet<&'a str>,
    visiting: &mut HashSet<&'a str>,
) -> Result<(), Box<dyn error::Error>> {
    if visiting.contains(module) {
        return Err(Box::new(CircularDependencyError {
            module_name: module.to_string(),
        }));
    }

    if !visited.contains(module) {
        visiting.insert(module);

        if let Some(deps) = dependencies.get(module) {
            for &dep in deps {
                if !dependencies.contains_key(dep) {
                    return Err(Box::new(MissingDependencyError {
                        module_name: module.to_string(),
                        dependency_name: dep.to_string(),
                    }));
                }
                visit(dep, dependencies, sorted, visited, visiting)?;
            }
        }

        visiting.remove(module);
        visited.insert(module);
        sorted.push(module);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::util::dependency::{
        sort_dependencies, CircularDependencyError, MissingDependencyError,
    };
    use std::collections::HashMap;

    #[test]
    fn test_sort_dependencies_success() {
        let dependencies: HashMap<&str, Vec<&str>> = HashMap::from([
            ("module1", vec!["module2", "module3"]),
            ("module2", vec!["module4"]),
            ("module3", vec![]),
            ("module4", vec![]),
        ]);

        let result = sort_dependencies(&dependencies);
        assert!(result.is_ok());
        let sorted = result.unwrap();

        // Verify the ordering
        assert!(
            sorted.iter().position(|&x| x == "module4")
                < sorted.iter().position(|&x| x == "module2")
        );
        assert!(
            sorted.iter().position(|&x| x == "module3")
                < sorted.iter().position(|&x| x == "module1")
        );
    }

    #[test]
    fn test_sort_dependencies_missing_dependency() {
        let dependencies: HashMap<&str, Vec<&str>> = HashMap::from([
            ("module1", vec!["module2", "module5"]), // module5 does not exist
            ("module2", vec!["module3"]),
            ("module3", vec![]),
        ]);

        let result = sort_dependencies(&dependencies);
        assert!(result.is_err());
        assert!(result.unwrap_err().is::<MissingDependencyError>());
    }

    #[test]
    fn test_sort_dependencies_circular_dependency() {
        let dependencies: HashMap<&str, Vec<&str>> = HashMap::from([
            ("module1", vec!["module2"]),
            ("module2", vec!["module3"]),
            ("module3", vec!["module1"]), // Circular dependency
        ]);

        let result = sort_dependencies(&dependencies);
        assert!(result.is_err());
        assert!(result.unwrap_err().is::<CircularDependencyError>());
    }

    #[test]
    fn test_sort_dependencies_no_dependencies() {
        let dependencies: HashMap<&str, Vec<&str>> = HashMap::from([
            ("module1", vec![]),
            ("module2", vec![]),
            ("module3", vec![]),
        ]);

        let result = sort_dependencies(&dependencies);
        assert!(result.is_ok());
        let sorted = result.unwrap();
        assert_eq!(sorted.len(), 3);
        assert!(sorted.contains(&"module1"));
        assert!(sorted.contains(&"module2"));
        assert!(sorted.contains(&"module3"));
    }

    #[test]
    fn test_sort_dependencies_single_module() {
        let dependencies: HashMap<&str, Vec<&str>> = HashMap::from([("module1", vec![])]);

        let result = sort_dependencies(&dependencies);
        assert!(result.is_ok());
        let sorted = result.unwrap();
        assert_eq!(sorted, vec!["module1"]);
    }
}
