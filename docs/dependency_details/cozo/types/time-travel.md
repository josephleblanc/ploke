==============
Time travel
==============

Simply put, time travel in a database means tracking changes to data over time 
and allowing queries to be logically executed at a point in time 
to get a historical view of the data. 
In a sense, this makes your database immutable, 
since nothing is really deleted from the database ever.
:doc:`This story <releases/v0.4>` gives some motivations why time travel may be valuable.

In Cozo, a stored relation is eligible for time travel if the last part of its key
has the explicit type ``Validity``.
A validity has two parts: a time part, represented by a signed integer,
and an assertion part, represented by a boolean, so ``[42, true]`` represents
a validity. Sorting of validity is by the timestamp first, then by the assertive flag,
but each field is compared in descending order, so::

    [1, true] < [1, false] < [-1, true]

All rows with identical key parts except the last validity part form
the *history* for that key, interpreted in the following way:
the fact represented by a row is *valid* if its flag is ``true``, and
the range of its validity is from its timestamp (inclusive) up until 
the timestamp of the next row under the same key (excluding the last validity part,
and here time is interpreted to flow forward). For example, let's say we have the rows ``'a', [10, true] => 'x'``, ``'a', [20, true] => 'y'``, ``'a', [30, true] => 'z'``, then at timestamps 10, 11, ..., 19, ``'a'`` is asserted to be ``'x'``, and at timestamps 20, 21, ..., 29, ``'a'`` is asserted to be ``'y'``, and from timestamp 30 onwards, ``'a'`` is asserted to be ``'z'``. Before timestamp 10, ``'a'`` doesn't exist.


A row with a ``false`` assertive flag does nothing other than 
making the previous fact invalid. For example, if we add to the above rows a row ``'a', [15, false] => null``, then ``'a'`` no longer exists at timestamps 15, 16, ..., 19, and at timestamp 20 onwards, ``'a'`` is asserted to be ``'y'``.

When querying against such a stored relation, a validity specification can be attached,
for example::

    ?[name] := *rel{id: $id, name, @ 789}

The part after the symbol ``@`` is the validity specification and must be a compile-time
constant, i.e., it cannot contain variables. Logically, it is as if
the query is against a snapshot of the relation containing only valid facts at the specified timestamp.

It is possible for two rows to have identical non-validity key parts and identical 
timestamps, but differ in their assertive flags. In this case when queried against
the exact timestamp, the row is valid, as if the row with the ``false`` flag
does not exist. For example, if we add to the above rows a row ``'a', [30, false] => null``,
since we already have a row ``'a', [30, true] => 'z'``, when queried against timestamp 30,
``'a'`` is asserted to be ``'z'``. The effect of the retraction is invisible from any time-travel query.

The use case for this behaviour is to assert a fact only until a future time
when that fact is sure to remain valid. When that time comes, a new fact can be asserted,
and if the old fact remains valid there is no need to ``:rm`` the previous retraction.

You can use the function ``to_bool`` to extract the flag of a validity, 
and ``to_int`` to extract the timestamp as an integer.

In Cozo it is up to you to interpret the timestamp part of validity. If you use it
to represent calendar time, then it is recommended that you treat it as microseconds since the
UNIX epoch. For this interpretation, the following convenience are provided:

* When putting facts into the database, instead of specifying the exact literal validity
  as a list of two items, the strings ``ASSERT`` and ``RETRACT`` can be used instead,
  and is interpreted as assertion and retraction at the current timestamp, respectively.
  This has the additional guarantee that all insertion operations in the same transaction
  using this method gets the same timestamp, and furthermore you can also use these strings
  as the default values for a field, and they will do the right thing.

* In place of a list of two items for specifying the literal validity, you can use
  RFC 3339 strings for assertion timestamps or validity specification in query. 
  For retraction, prefix the string by ``~``.

* When specifying validity against a stored relation, the string ``NOW`` uses the current timestamp,
  and ``END`` uses a timestamp logically at the end of the world. Furthermore, the ``NOW`` timestamp
  is guaranteed to be the same as what would be inserted using ``ASSERT`` and ``RETRACT``.

* You can use the function ``format_timestamp`` to directly format a the timestamp part of a validity to
  RFC 3339 strings.

An interesting use case of the time travel facility is to pre-generate the whole history for all time,
and in the user-facing interface query with the current time ``NOW``.
The effect is that users see an illusion of real-time interactions:
a manifestation of `Laplace's daemon <https://en.wikipedia.org/wiki/Laplace%27s_demon>`_.
