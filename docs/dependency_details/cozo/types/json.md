------------------------
Json funcitons
------------------------

.. function:: json(x)

    Converts any value to a Json value. This function is idempotent and never fails.

.. function:: is_json(x)

    Returns ``true`` if the argument is a Json value, ``false`` otherwise.

.. function:: json_object(k1, v1, ...)

    Convert a list of key-value pairs to a Json object.

.. function:: dump_json(x)

    Convert a Json value to its string representation.

.. function:: parse_json(x)

    Parse a string to a Json value.


.. function:: get(json, idx, default?)

    Returns the element at index ``idx`` in the Json ``json``. 
    
    ``idx`` may be a string (for indexing objects), a number (for indexing arrays), or a list of strings and numbers (for indexing deep structures).
    
    Raises an error if the requested element cannot be found, unless ``default`` is specified, in which cast ``default`` is returned.

.. function:: maybe_get(json, idx)

    Returns the element at index ``idx`` in the Json ``json``. Same as ``get(json, idx, null)``. The shorthand is ``json->idx``.


.. function:: set_json_path(json, path, value)

    Set the value at the given path in the given Json value. The path is a list of keys of strings (for indexing objects) or numbers (for indexing arrays). The value is converted to Json if it is not already a Json value.

.. function:: remove_json_path(json, path)

    Remove the value at the given path in the given Json value. The path is a list of keys of strings (for indexing objects) or numbers (for indexing arrays).

.. function:: json_to_scalar(x)

    Convert a Json value to a scalar value if it is a ``null``, boolean, number or string, and returns the argument unchanged otherwise.

.. function:: concat(x, y, ...)

    Concatenate (deep-merge) Json values. It is equivalent to the operator form ``x ++ y ++ ...``

    The concatenation of two Json arrays is the concatenation of the two arrays. The concatenation of two Json objects is the deep-merge of the two objects, meaning that their key-value pairs are combined, with any pairs that appear in both left and right having their values deep-merged. For all other cases, the right value wins.
