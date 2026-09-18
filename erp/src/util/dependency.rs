use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("Missing dependency '{dependency_name}' for plugin '{plugin_name}'")]
pub struct MissingDependencyError {
    pub(crate) plugin_name: String,
    pub(crate) dependency_name: String,
}

#[derive(Debug, Clone, Error)]
#[error("Circular dependency detected for plugin '{plugin_name}'")]
pub struct CircularDependencyError {
    pub(crate) plugin_name: String,
}

pub fn sort_dependencies<'a>(
    dependencies: &HashMap<&'a str, Vec<&str>>,
) -> Result<Vec<&'a str>, Box<dyn std::error::Error + Send + Sync>> {
    let mut sorted = Vec::new();
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();

    for &plugin in dependencies.keys() {
        if !visited.contains(plugin) {
            visit(
                plugin,
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
    plugin: &'a str,
    dependencies: &HashMap<&'a str, Vec<&str>>,
    sorted: &mut Vec<&'a str>,
    visited: &mut HashSet<&'a str>,
    visiting: &mut HashSet<&'a str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if visiting.contains(plugin) {
        return Err(Box::new(CircularDependencyError {
            plugin_name: plugin.to_string(),
        }));
    }

    if !visited.contains(plugin) {
        visiting.insert(plugin);

        if let Some(deps) = dependencies.get(plugin) {
            for &dep in deps {
                if let Some((key, _)) = dependencies.get_key_value(dep) {
                    visit(key, dependencies, sorted, visited, visiting)?;
                } else {
                    return Err(Box::new(MissingDependencyError {
                        plugin_name: plugin.to_string(),
                        dependency_name: dep.to_string(),
                    }));
                }
            }
        }

        visiting.remove(plugin);
        visited.insert(plugin);
        sorted.push(plugin);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::util::dependency::{
        CircularDependencyError, MissingDependencyError, sort_dependencies,
    };
    use std::collections::HashMap;

    #[test]
    fn test_sort_dependencies_success() {
        let dependencies: HashMap<&str, Vec<&str>> = HashMap::from([
            ("plugin1", vec!["plugin2", "plugin3"]),
            ("plugin2", vec!["plugin4"]),
            ("plugin3", vec![]),
            ("plugin4", vec![]),
        ]);

        let result = sort_dependencies(&dependencies);
        assert!(result.is_ok());
        let sorted = result.unwrap();

        // Verify the ordering
        assert!(
            sorted.iter().position(|&x| x == "plugin4")
                < sorted.iter().position(|&x| x == "plugin2")
        );
        assert!(
            sorted.iter().position(|&x| x == "plugin3")
                < sorted.iter().position(|&x| x == "plugin1")
        );
    }

    #[test]
    fn test_sort_dependencies_missing_dependency() {
        let dependencies: HashMap<&str, Vec<&str>> = HashMap::from([
            ("plugin1", vec!["plugin2", "plugin5"]), // plugin5 does not exist
            ("plugin2", vec!["plugin3"]),
            ("plugin3", vec![]),
        ]);

        let result = sort_dependencies(&dependencies);
        assert!(result.is_err());
        assert!(result.unwrap_err().is::<MissingDependencyError>());
    }

    #[test]
    fn test_sort_dependencies_circular_dependency() {
        let dependencies: HashMap<&str, Vec<&str>> = HashMap::from([
            ("plugin1", vec!["plugin2"]),
            ("plugin2", vec!["plugin3"]),
            ("plugin3", vec!["plugin1"]), // Circular dependency
        ]);

        let result = sort_dependencies(&dependencies);
        assert!(result.is_err());
        assert!(result.unwrap_err().is::<CircularDependencyError>());
    }

    #[test]
    fn test_sort_dependencies_no_dependencies() {
        let dependencies: HashMap<&str, Vec<&str>> = HashMap::from([
            ("plugin1", vec![]),
            ("plugin2", vec![]),
            ("plugin3", vec![]),
        ]);

        let result = sort_dependencies(&dependencies);
        assert!(result.is_ok());
        let sorted = result.unwrap();
        assert_eq!(sorted.len(), 3);
        assert!(sorted.contains(&"plugin1"));
        assert!(sorted.contains(&"plugin2"));
        assert!(sorted.contains(&"plugin3"));
    }

    #[test]
    fn test_sort_dependencies_single_plugin() {
        let dependencies: HashMap<&str, Vec<&str>> = HashMap::from([("plugin1", vec![])]);

        let result = sort_dependencies(&dependencies);
        assert!(result.is_ok());
        let sorted = result.unwrap();
        assert_eq!(sorted, vec!["plugin1"]);
    }
}
