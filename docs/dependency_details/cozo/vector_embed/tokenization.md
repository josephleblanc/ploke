# On Tokenization

From the official [cozo documentation](https://docs.cozodb.org/en/latest/vector.html)
Available also from the [cozo docs github](https://github.com/cozodb/cozo-docs/blob/main/source/vector.rst)

============================================================
Proximity searches
============================================================

These kinds of proximity indices allow Cozo to perform fast searches for similar data. The HNSW index is a graph-based index that allows for fast approximate nearest neighbor searches. The MinHash-LSH index is a locality sensitive hash index that allows for fast approximate nearest neighbor searches. The FTS index is a full-text search index that allows for fast string matches.

--------------------------------------------------------------
HNSW (Hierarchical Navigable Small World) indices for vectors
--------------------------------------------------------------

Cozo supports vector proximity search using the HNSW (Hierarchical Navigable Small World) algorithm. 

To use vector search, you first need to have a stored relation with vectors inside, for example::

    :create table {k: String => v: <F32; 128>}


Next you create a HNSW index on a table containing vectors. You use the following system operator to create the index::

    ::hnsw create table:index_name {
        dim: 128,
        m: 50,
        dtype: F32,
        fields: [v],
        distance: L2,
        ef_construction: 20,
        filter: k != 'foo',
        extend_candidates: false,
        keep_pruned_connections: false,
    }

The parameters are as follows:

- The dimension ``dim`` and the data type ``dtype`` (defaults to F32) has to match the dimensions of any vector you index.
- The ``fields`` parameter is a list of fields in the table that should be indexed.
- The indexed fields must only contain vectors of the same dimension and data type, or ``null``, or a list of vectors of the same dimension and data type.
- The ``distance`` parameter is the distance metric to use: the options are ``L2`` (default), ``Cosine`` and ``IP``.
- The ``m`` controls the maximal number of outgoing connections from each node in the graph.
- The ``ef_construction`` parameter is the number of nearest neighbors to use when building the index: see the HNSW paper for details.
- The ``filter`` parameter, when given, is bound to the fields of the original relation and only those rows for which the expression evaluates to ``true`` are indexed.
- The ``extend_candidates`` parameter is a boolean (default false) that controls whether the index should extend the candidate list with the nearest neighbors of the nearest neighbors.
- The ``keep_pruned_connections`` parameter is a boolean (default false) that controls whether the index should keep pruned connections.

You can insert data as normally done into ``table``. For vectors, use a list of numbers and it will be verified to have the correct dimension and converted. If you want to be more explicit, you can use the ``vec`` function.

After the index is created, you can use vector search inside normal queries in a similar manner to stored relations. For example::

    ?[dist, k, v] := ~table:index_name{ k, v | 
            query: q, 
            k: 2, 
            ef: 20, 
            bind_distance: dist, 
            bind_vector: bv, 
            bind_field: f, 
            bind_field_idx: fi, 
            filter: 1 + 1 == 2,
            radius: 0.1
        }, q = vec([200, 34])

The ``~`` followed by the index name signifies a vector search. In the braces, arguments before the vertical line are named bindings, with exactly the same semantics as in normal stored relations with named fields (i.e. they may be bound, or if they are unbound, the introduce fresh variables), and arguments after the vertical line are query parameters.

There are three required parameters: ``query`` is an expression that evaluates to a query vector of the expected type, and if it evaluates to a variable, the variable must be bound inside the rule; ``k`` controls how many results to return, and ``ef`` controls the number of neighbours to consider during the search process.

Next, there are three bind parameters that can bind variables to data that are only available in index or during the search process: ``distance`` binds the distance between the query vector and the result vector; ``vector`` binds the result vector; and ``field`` binds the field name of the result vector. The ``field_idx`` parameter binds the index of the field in the ``fields`` list of the index in case ``field`` resolves to a list of vectors, otherwise it is ``null``. In case any of the bind parameters are bound to existing variables, they act as filters after ``k`` results are returned.

The parameter ``filter`` takes an expression that can only refer to the fields of the original relation, and only those rows for which the expression evaluates to ``true`` are returned, and this filtering results occurs during the search process, so the algorithm will strive to return ``k`` results even if it must filter out a larger number of rows. ``radius`` controls the largest distance any return vector can have from the query vector, and this filtering process also happens during the search.

The vector search can be used in any place where a stored relation may be used, even inside recursive rules (but be careful of non-termination).

As with normal indices, you can use the index relation as a read-only but otherwise normal relation in your query. You query the index directly by::

    ?[fr_k, to_k, dist] := *table:index_name {layer: 0, fr_k, to_k, dist}

It is recommended to always specify ``layer``, otherwise a full scan is required.

The schema for the above index is the following::

    {
        layer: Int,
        fr_k: String?,
        fr__field: Int?,
        fr__field_idx: Int?,
        to_k: String?,
        to__field: Int?,
        to__field_idx: Int?,
        => 
        dist: Float,
        hash: Bytes,
        ignore_link: Bool,
    }

Layer is the layer in the HNSW hierarchy of graphs, with ``0`` the most detailed layer, ``-1`` the layer more abstract than ``0``, ``-2`` the even more abstract layer, etc. There is also a special layer ``1`` containing at most one row with all other keys set to null.

The ``fr_*`` and ``to_*`` fields mirror the indices of the indexed relation, and the ``fr__*`` and ``to__*`` fields indicate which vectors inside the original rows this edge connects.

``dist`` is the distance between the two vectors when the row represents a link between two different vectors, otherwise the link is a self-loop and ``dist`` contains the degree of the node; ``hash`` is the hash of the vector, and ``ignore_link`` is a boolean that indicates whether this link should be ignored during the search process. The graph is guaranteed to be symmetric, but the incoming and outgoing links may have different ``ignore_link`` values, and they cannot both be ``true``.

Walking the index graph at layer 0 amounts to probabilistically visiting "near" neigbours. More abstract layers are renormalized versions of the proximity graph and are harder to work with but are even more interesting theoretically.

To drop an HNSW index::

    ::hnsw drop table:index_name

--------------------------------------------------------------
MinHash-LSH for near-duplicate indexing of strings and lists
--------------------------------------------------------------

To use locality sensitive search on a relation containing string values, for example::

    :create table {k: String => v: String?}

You can create a MinHash-LSH index on the ``v`` field by::

    ::lsh create table:index_name {
        extractor: v,
        extract_filter: !is_null(v),
        tokenizer: Simple,
        filters: [],
        n_perm: 200,
        target_threshold: 0.7,
        n_gram: 3,
        false_positive_weight: 1.0,
        false_negative_weight: 1.0,
    }

This creates a MinHash-LSH index on the ``v`` field of the table. The index configuration includes the following parameters:

- ``extractor: v`` specifies that the ``v`` field will be used as the feature extractor. This parameter takes an expression, which must evaluate to a string, a list of values to be indexed, or ``null``. If it evaluates to ``null``, then the row is not indexed.
- ``extract_filter: !is_null(v)``: this is superfluous in this case, but in more general situations you can use this to skip indexing rows based on arbitary logic.
- ``tokenizer: Simple`` and ``filters: []`` specifies the tokenizer to be used, see a later section for tokenizer.
- ``n_perm: 200`` sets the number of permutations for the MinHash LSH index. Higher values will result in more accurate results at the cost of increased CPU and storage usage.
- ``target_threshold: 0.7`` sets the target threshold for similarity comparisons when searching.
- ``n_gram: 3`` sets the size of the n-gram used for `shingling <https://en.wikipedia.org/wiki/W-shingling>`_.
- ``false_positive_weight: 1.0`` and ``false_negative_weight: 1.0`` set the weights for false positives and false negatives.

At search time::

    ?[k, v] := ~table:index_name {k, v | 
        query: $q, 
        k: 2, 
        filter: 1 + 1 == 2, 
    }

This will look for the top 2 most similar values to the query ``q``. The ``filter`` parameter is evaluated on the bindings for the relation, and only those rows for which the filter evaluates to ``true`` are returned, before restricting to ``k`` results. The ``query`` parameter is a string, and will be subject to the same tokenization process.

In addition to strings, you can index and search for list of arbitrary values. In this case, the ``tokenizer``, ``filters`` and ``n_gram`` parameters are ignored.

Again you can use the associated index relation as a normal relations in your query. There are two now: ``table:index_name`` and ``table:index_name:inv``. You can use ``::columns`` to look at their structure. In our case, the first is::

    {
        hash: Bytes,
        src_k: String,
    }

and the second is::

    {
        k: String => minhash: List[Bytes]
    }

The first it more useful: it loosely groups together duplicates according to the indexing parameters.

To drop::

    ::lsh drop table:index_name

--------------------------------------------------------------
Full-text search (FTS)
--------------------------------------------------------------

Full-text search should be familiar. For the following relation::

    :create table {k: String => v: String?}

we can create an FTS index by::

    ::fts create table:index_name {
        extractor: v,
        extract_filter: !is_null(v),
        tokenizer: Simple,
        filters: [],
    }

This creates an FTS index on the ``v`` field of the table. The index configuration includes the following parameters:

- ``extractor: v`` specifies that the ``v`` field will be used as the feature extractor. This parameter takes an expression, which must evaluate to a string or ``null``. If it evaluates to ``null``, then the row is not indexed.
- ``extract_filter: !is_null(v)``: this is superfluous in this case, but in more general situations you can use this to skip indexing rows based on arbitary logic.
- ``tokenizer: Simple`` and ``filters: []`` specifies the tokenizer to be used, see a later section for tokenizer.

That's it. At query time::

    ?[s, k, v] := ~table:index_name {k, v | 
        query: $q, 
        k: 10, 
        filter: 1 + 1 == 2,
        score_kind: 'tf_idf',
        bind_score: s
    }
    
    :order -s

This query retrieves the top 10 results from the index ``index_name`` based on a search query ``$q``. The ``filter`` parameter can be used to filter the results further based on additional conditions. The ``score_kind`` parameter specifies the scoring method, and in this case, it is set to ``'tf_idf'`` which takes into consideration of global statistics when scoring documents. You can also use ``'tf'``. The resulting scores are bound to the variable ``s``. Finally, the results are ordered in descending order of score (``-s``).

The search query must be a string and is processed by the same tokenizer as the index. The tokenizer is specified by the ``tokenizer`` parameter, and the ``filters`` parameter can be used to specify additional filters to be applied to the tokens. There is a mini-language for parsing the query:

- ``hello world``, ``hello AND world``, ``"hello" AND 'world'``: these all look for rows where both words occur. ``AND`` is case sensitive.
- ``hello OR world``: look for rows where either word occurs.
- ``hello NOT world``: look for rows where ``hello`` occurs but ``world`` does not.
- ``hell* wor*``: look for rows having a word starting with ``hell`` and also a word starting with ``wor``.
- ``NEAR/3(hello world bye)``: look for rows where ``hello``, ``world``, ``bye`` are within 3 words of each other. You can write ``NEAR(hello world bye)`` as a shorthand for ``NEAR/10(hello world bye)``.
- ``hello^2 OR world``: look for rows where ``hello`` or ``world`` occurs, but ``hello`` has twice of its usual weighting when scoring.
- These can be combined and nested with parentheses (except that ``NEAR`` only takes literals and prefixes): ``hello AND (world OR bye)``.

The index relation has the following schema::

    {
        word: String,
        src_k: String,
        =>
        offset_from: List[Int],
        offset_to: List[Int],
        position: List[Int],
        total_length: Int,
    }

Explanation of the fields:

- ``word``: the word that occurs in the document.
- ``src_k``: the key of the document, the name and number varies according to the original relation schema.
- ``offset_from``: the starting offsets of the word in the document.
- ``offset_to``: the ending offsets of the word in the document.
- ``position``: the position of the word in the document, counted as the position of entire tokens.
- ``total_length``: the total number of tokens in the document.

To drop::

    ::fts drop table:index_name

----------------------------------
Text tokenization and filtering
----------------------------------

Text tokenization and filtering are used in both the MinHash-LSH and FTS indexes. The tokenizer is specified by the ``tokenizer`` parameter, and the ``filters`` parameter can be used to specify additional filters to be applied to the tokens.

CozoDB uses `Tantivy's <https://github.com/quickwit-oss/tantivy>`_ tokenizers and filters (we incorporated their files directly in our source tree, as they are not available as a library). Tokenizer is specified in the configuration as a function call such as ``Ngram(9)``, or if you omit all arguments, ``Ngram`` is also acceptable. The following tokenizers are available:

- ``Raw``: no tokenization, the entire string is treated as a single token.
- ``Simple``: splits on whitespace and punctuation.
- ``Whitespace``: splits on whitespace.
- ``Ngram(min_gram?, max_gram?, prefix_only?)``: splits into n-grams. ``min_gram`` is the minimum size of the n-gram (default 1), ``max_gram`` is the maximum size of the n-gram (default to ``min_gram``), and ``prefix_only`` is a boolean indicating whether to only generate prefixes of the n-grams (default false).
- ``Cangjie(kind?)``: this is a text segmenter for the Chinese language. ``kind`` can be ``'default'``, ``'all'``, ``'search'`` or ``'unicode'``.

After tokenization, multiple filters can be applied to the tokens. The following filters are available:

- ``Lowercase``: converts all tokens to lowercase.
- ``AlphaNumOnly``: removes all tokens that are not alphanumeric.
- ``AsciiFolding``: converts all tokens to ASCII (lossy), i.e. ``passé`` goes to ``passe``.
- ``Stemmer(lang)``: use a language-specific stemmer. The following languages are available: ``'arabic'``, ``'danish'``, ``'dutch'``, ``'english'``, ``'finnish'``, ``'french'``, ``'german'``, ``'greek'``, ``'hungarian'``, ``'italian'``, ``'norwegian'``, ``'portuguese'``, ``'romanian'``, ``'russian'``, ``'spanish'``, ``'swedish'``, ``'tamil'``, ``'turkish'``. As an exmple, the English stemmer converts ``'running'`` to ``'run'``.
- ``Stopwords(lang)``: filter out stopwords specific to the language. The stopwords come from the `stopwords-iso <https://github.com/stopwords-iso/stopwords-iso>`_ project. Use the ISO 639-1 Code as specified on the project page. For example, ``Stopwords('en')`` for English will remove words such as ``'the'``, ``'a'``, ``'an'``, etc.

For English text, the recommended setup is ``Simple`` for the tokenizer and ``[Lowercase, Stemmer('english'), Stopwords('en')]`` for the filters.
