# Contributing to StateSet API

First off, thank you for considering contributing to StateSet API! It's people like you that make StateSet API such a great tool. 🎉

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [How Can I Contribute?](#how-can-i-contribute)
- [Development Process](#development-process)
- [Style Guidelines](#style-guidelines)
- [Community](#community)

## Code of Conduct

This project and everyone participating in it is governed by our Code of Conduct. By participating, you are expected to uphold this code. Please report unacceptable behavior to [support@stateset.com](mailto:support@stateset.io).

## Getting Started

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- PostgreSQL 14+
- Redis 6+
- Protocol Buffers compiler (`protoc`)
- Git

### Setting Up Your Development Environment

1. **Fork the repository**
   ```bash
   # Click the 'Fork' button on GitHub, then:
   git clone https://github.com/YOUR_USERNAME/stateset-api.git
   cd stateset-api
   git remote add upstream https://github.com/stateset/stateset-api.git
   ```

2. **Install dependencies**
   ```bash
   # Install Rust dependencies
   cargo build
   
   # Install protoc (if not already installed)
   # macOS
   brew install protobuf
   
   # Ubuntu/Debian
   sudo apt-get install protobuf-compiler
   
   # Or download from the project
   ./install-protoc.sh
   ```

3. **Set up the database**
   ```bash
   # Start PostgreSQL and Redis
   docker-compose up -d postgres redis
   
   # Or use your local installations
   createdb stateset_dev
   ```

4. **Configure environment**
   ```bash
   # Copy the example environment file
   cp ENV_VARIABLES.md .env
   # Edit .env with your local settings
   ```

5. **Run migrations**
   ```bash
   cargo run --bin migration up
   ```

6. **Run the development server**
   ```bash
   cargo run --bin api_server
   ```

7. **Run tests**
   ```bash
   # Run all tests
   cargo test
   
   # Run with coverage
   cargo tarpaulin --out Html
   ```

## How Can I Contribute?

### Reporting Bugs

Before creating bug reports, please check the existing issues as you might find out that you don't need to create one. When you are creating a bug report, please include as many details as possible:

**Bug Report Template:**
```markdown
**Describe the bug**
A clear and concise description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Configure '...'
2. Send request to '...'
3. See error

**Expected behavior**
What you expected to happen.

**Actual behavior**
What actually happened. Include error messages and stack traces.

**Environment:**
 - OS: [e.g. Ubuntu 22.04]
 - Rust version: [e.g. 1.70.0]
 - Database: [e.g. PostgreSQL 14.5]
 
**Additional context**
Any other context about the problem.
```

### Suggesting Enhancements

Enhancement suggestions are tracked as GitHub issues. When creating an enhancement suggestion, please include:

- **Use a clear and descriptive title**
- **Provide a step-by-step description** of the suggested enhancement
- **Provide specific examples** to demonstrate the steps
- **Describe the current behavior** and explain which behavior you expected to see instead
- **Explain why this enhancement would be useful**

### Your First Code Contribution

Unsure where to begin contributing? You can start by looking through these issues:

- Issues labeled `good first issue` - issues which should only require a few lines of code
- Issues labeled `help wanted` - issues which need extra attention
- Issues labeled `documentation` - improvements or additions to documentation

### Pull Requests

1. **Create a topic branch** from where you want to base your work
   ```bash
   git checkout -b feature/my-new-feature
   ```

2. **Make your changes**
   - Write clear, concise commit messages
   - Include tests for new functionality
   - Update documentation as needed

3. **Follow the style guidelines** (see below)

4. **Ensure all tests pass**
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

5. **Push to your fork** and submit a pull request

6. **PR Guidelines:**
   - Fill in the required template
   - Do not include issue numbers in the PR title
   - Include screenshots and animated GIFs in your pull request whenever possible
   - End all files with a newline

## Development Process

### Branch Organization

- `main` - stable release branch
- `develop` - main development branch
- `feature/*` - feature branches
- `fix/*` - bug fix branches
- `release/*` - release preparation branches

### Commit Messages

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation only changes
- `style:` - Code style changes (formatting, etc)
- `refactor:` - Code change that neither fixes a bug nor adds a feature
- `perf:` - Performance improvement
- `test:` - Adding missing tests
- `chore:` - Changes to the build process or auxiliary tools

**Examples:**
```bash
feat(orders): add bulk order import endpoint
fix(inventory): correct stock level calculation
docs(api): update authentication examples
```

### Testing

- Write unit tests for all new functionality
- Maintain or increase code coverage
- Include integration tests for API endpoints
- Test error cases, not just happy paths

Example test:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_order() {
        // Arrange
        let db = setup_test_db().await;
        let service = OrderService::new(db);
        
        // Act
        let order = service.create_order(/* ... */).await;
        
        // Assert
        assert!(order.is_ok());
        assert_eq!(order.unwrap().status, OrderStatus::Pending);
    }
}
```

## Style Guidelines

### Rust Style

We use `rustfmt` and `clippy` to maintain consistent code style:

```bash
# Format code
cargo fmt

# Check linting
cargo clippy -- -D warnings
```

**Key guidelines:**
- Use descriptive variable names
- Keep functions small and focused
- Document public APIs with doc comments
- Use `Result<T, E>` for error handling
- Prefer `&str` over `String` for function parameters
- Use `Arc<T>` for shared ownership across threads

### API Design

- Follow RESTful principles
- Use consistent naming conventions
- Version APIs appropriately (`/api/v1/`)
- Return appropriate HTTP status codes
- Include helpful error messages

### Documentation

- Document all public functions and types
- Include examples in doc comments
- Keep README.md up to date
- Document breaking changes

Example:
```rust
/// Creates a new order with the specified items.
///
/// # Arguments
///
/// * `customer_id` - The ID of the customer placing the order
/// * `items` - Vector of items to include in the order
///
/// # Examples
///
/// ```
/// let order = create_order(customer_id, vec![item1, item2]).await?;
/// ```
///
/// # Errors
///
/// Returns `OrderError::InvalidCustomer` if the customer doesn't exist.
pub async fn create_order(
    customer_id: Uuid,
    items: Vec<OrderItem>
) -> Result<Order, OrderError> {
    // Implementation
}
```

### Database Schema

- Use migrations for all schema changes
- Name tables in plural form (e.g., `orders`, `customers`)
- Use `snake_case` for column names
- Include timestamps (`created_at`, `updated_at`)
- Add appropriate indexes

## Pull Request Process

1. **Before submitting:**
   - Rebase on the latest `main` branch
   - Ensure all tests pass
   - Update documentation if needed
   - Add entry to CHANGELOG.md if applicable

2. **PR description should include:**
   - What changes were made
   - Why these changes were made
   - How to test the changes
   - Screenshots (if UI changes)

3. **Review process:**
   - At least one maintainer approval required
   - All CI checks must pass
   - No merge conflicts
   - Conversations resolved

4. **After merge:**
   - Delete your branch
   - Update your local repository

## Community

### Getting Help

- **Discord**: Join our [Discord server](https://discord.gg/stateset)
- **Discussions**: Use [GitHub Discussions](https://github.com/stateset/stateset-api/discussions) for questions
- **Stack Overflow**: Tag questions with `stateset`

### Recognition

Contributors are recognized in our:
- [Contributors list](https://github.com/stateset/stateset-api/contributors)
- Release notes
- Annual contributor spotlight

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (Business Source License 1.1).

## Questions?

Don't hesitate to ask questions! We're here to help. You can:
- Open an issue with the `question` label
- Ask in our Discord server
- Email us at support@stateset.com

Thank you for contributing to StateSet API! 🚀 