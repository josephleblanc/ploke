# Nice to have features

1. Quick and easy way to see what is currently in context.
  - should be a new overlay with a list of items, similar to how the model selection overlay works
  - initially list should just be populated with the names of the included items, e.g. for the function `some_func` just the function signature or name
  - on hitting `l` or `Enter`, the item should expand to a code snippet of the referenced item
  - if the item is expanded already, hitting `h` or `Enter` should collapse the item

It would also be nice if this context viewer had these features, but they are lower priority:
  - using `r` to remove the item from context.
  - using `s` to keep the item in the context so the rolling window does not remove it
