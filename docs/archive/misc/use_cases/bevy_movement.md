# Bevy Player Movement Use Case

## Overview
This document details how ploke's RAG system would handle a request for "player movement with WASD keys" when analyzing both the user's code and the Bevy game engine dependency.

## Workflow

### 1. Dependency Configuration
User specifies Bevy as an analyzed dependency:
```toml
[ploke.dependencies]
bevy = { version = "0.13", analyze = true }
```

### 2. Database Population (Bevy ECS)
Key structures stored:

1. **Component Definitions**:
```cozo
?[id, name, visibility, docstring] <- [
    [101, "Transform", "Public", "3D transform component..."],
    [102, "KeyCode", "Public", "Keyboard key codes..."]
]
:put structs
```

2. **System Patterns**:
```cozo
?[id, name, params] <- [
    [201, "query_movement", "Query<&mut Transform, With<Player>>"],
    [202, "keyboard_input", "Res<Input<KeyCode>>"]
]
:put system_patterns
```

### 3. Query Processing Pipeline

#### Semantic Search
```cozo
?[text, score] := 
    *code_embeddings[_, "bevy", "docstring", _, text],
    ~text_search:search{query: "WASD movement", bind_distance: score},
    score > 0.7
```

#### Context Retrieval
```cozo
?[example, context] :=
    *bevy_examples[name, example, context],
    *relations[name, "PlayerMovement", "ExampleOf"],
    *relations["PlayerMovement", "Transform", "Mutates"]
```

### 4. Generated Context
```markdown
Bevy 0.13 Movement Context:
1. Input System:
```rust
fn keyboard_movement(
    keys: Res<Input<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>
) {
    let mut transform = query.single_mut();
    let speed = 2.0;
    
    if keys.pressed(KeyCode::W) {
        transform.translation.y += speed;
    }
    //...
}
```

2. Best Practices:
- Always multiply by delta_time
- Use `With<Player>` to filter queries
- Cache query results when possible

3. Your Project:
```rust
#[derive(Component)] 
struct Player {
    speed: f32
}
```

## Key Database Structures

### Bevy-Specific Relations
```cozo
:create bevy_relations {
    source: String,
    target: String,
    kind: String =>
}
```

### Versioned Examples
```cozo
:create bevy_examples {
    name: String,
    code: String,
    context: String,
    version: String =>
}
```

## Open Questions
1. How to handle breaking changes between Bevy versions?
2. Should we store entire example files or just relevant snippets?
3. How to weight project-specific patterns vs dependency patterns?
