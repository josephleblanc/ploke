-- Qwen model --

To create effective templates for prompting an LLM to break down a requested task into a set of subtasks, we need to ensure that the templates are clear, structured, and adaptable to different types of tasks. The templates should also account for the possibility of using a `cfg` flag for changes that would otherwise break the codebase.

### Template 1: Simple Task Breakdown

**Description:** This template is straightforward and focuses on breaking down the task into simple, manageable subtasks. It is suitable for tasks that do not require complex changes or the use of `cfg` flags.

**Template:**
```
Task: [User Request]

Subtasks:
1. [Subtask 1]
2. [Subtask 2]
3. [Subtask 3]
...
```

**Example:**
```
Task: Ensure all publicly exposed types are `Send + Sync`.

Subtasks:
1. Identify all publicly exposed types in the `syn_parser` crate.
2. Add `Send + Sync` bounds to the identified types.
3. Run tests to ensure no regressions.
4. Commit changes.
```

### Template 2: Task Breakdown with `cfg` Flag

**Description:** This template is more complex and includes the use of a `cfg` flag for changes that would otherwise break the codebase. It is suitable for tasks that require significant refactoring or changes that could introduce errors.

**Template:**
```
Task: [User Request]

Subtasks:
1. Identify the changes required to fulfill the task.
2. Introduce a `cfg` flag named `feature_name` to encapsulate the changes.
3. Implement the changes under the `cfg(feature_name)` flag.
4. Test the changes with the `feature_name` flag enabled.
5. If tests pass, remove the `cfg(feature_name)` flag.
6. Commit changes.
```

**Example:**
```
Task: Change the types used within `syn_parser` to align with the CozoDB schema.

Subtasks:
1. Identify all types that need to be changed to align with the CozoDB schema.
2. Introduce a `cfg` flag named `cozodb_schema` to encapsulate the changes.
3. Implement the changes under the `cfg(cozodb_schema)` flag.
4. Test the changes with the `cozodb_schema` flag enabled.
5. If tests pass, remove the `cfg(cozodb_schema)` flag.
6. Commit changes.
```

### Template 3: Detailed Task Breakdown with `cfg` Flag and Documentation

**Description:** This template is the most detailed and includes the use of a `cfg` flag, as well as documentation of the changes. It is suitable for complex tasks that require thorough documentation and careful testing.

**Template:**
```
Task: [User Request]

Subtasks:
1. Identify the changes required to fulfill the task.
2. Introduce a `cfg` flag named `feature_name` to encapsulate the changes.
3. Implement the changes under the `cfg(feature_name)` flag.
4. Document the changes in the `CHANGELOG.md` and `README.md`.
5. Write unit tests for the changes.
6. Test the changes with the `feature_name` flag enabled.
7. If tests pass, remove the `cfg(feature_name)` flag.
8. Commit changes and update the documentation.
```

**Example:**
```
Task: Remove `CodeGraph` (or significantly reduce its scope).

Subtasks:
1. Identify all usages of `CodeGraph` in the `syn_parser` crate.
2. Introduce a `cfg` flag named `remove_codegraph` to encapsulate the changes.
3. Implement the changes under the `cfg(remove_codegraph)` flag.
4. Document the changes in the `CHANGELOG.md` and `README.md`.
5. Write unit tests for the changes.
6. Test the changes with the `remove_codegraph` flag enabled.
7. If tests pass, remove the `cfg(remove_codegraph)` flag.
8. Commit changes and update the documentation.
```

### Reasoning for Choices

1. **Template 1** is simple and straightforward, making it easy to understand and implement. It is suitable for tasks that do not require complex changes or the use of `cfg` flags.
2. **Template 2** introduces the use of a `cfg` flag, which is necessary for tasks that could break the codebase if implemented all at once. This template is more complex and includes a step-by-step approach to ensure the changes are tested and validated.
3. **Template 3** is the most detailed and includes documentation and unit tests. It is suitable for complex tasks that require thorough documentation and careful testing to ensure the changes do not introduce regressions.

These templates provide a structured approach to breaking down tasks into manageable subtasks, with varying levels of complexity and detail, to ensure that the LLM can generate a clear and actionable list of tasks.
