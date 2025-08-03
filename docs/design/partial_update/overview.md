# Partial Update Pipeline

## High level overview
Assumes that the target directory has been parsed, inserted into the database, and embeddings have been correctly generated and added to the primary nodes.

Once the initial database construction with embeddings is complete, we need to update the embeddings of the changed nodes. While reparsing the entire directory is relatively fast, embeddings take time and we want to minimize the number of embeddings we need to generate.

During this first implementation, we will use a broad brush to update all the nodes within the changed files. For now, we will only update the embeddings when the user submits a query to the database, but it is likely that we will try to make these updates more frequently to avoid the increased latency of generating embeddings in the user query pipeline.

Due to the way our parsing is currently implemented, we will need to change the database insertion process to update the given nodes in such a way that they do not overwrite the previous embeddings for files that are not in the changed files. Here is how we can do it:

1. Submit a list of files to a transform process that only inserts the nodes in the changed files to the database.
2. Instead of overwriting those nodes entirely, we call the `:update` cozo command to update the target nodes, specifically not including the embedding field in those fields that will be updated.
3. Prune any nodes in the database that are within the target files but have not been updated.

Step 3 above is the most tricky to implement correctly. If a given node is a child of the target file, but is not in the set of updated nodes, then it must either have been removed from the database or have been moved to a different file. We are not going to handle the case of the node having been moved to a different file, not in the near future, because this is a more edge case and would be complex to implement due to the use of the filename in generating the Canonical Uuid. 

As for the rest of point 3, the primary point of difficulty is that we want to provide time-series data on the code graph, meaning we want to retain the changed nodes in the database. However, we do not want those nodes to appear in the RAG search after they have been removed from the target file - but we do want to keep them in the database because there is possible use for them in analyzing the code evolution (modeling how the code graph changes over time). And while code evolution is not our current focus, we do want to enable this functionality in the future, or at least not footgun ourselves by architecting a solution that makes this more difficult.

There are two versions of plans below, so far I favor the second plan, and the first will be a backup if we have some problem with the implementation of the second plan or need to revisit this process sometime later.

### Plan 1
Our immediate plan for a dataflow here is:
1. Scan the files in the target directory, checking for changes in tracking hash
  - return a list of changed files
  - Q: What if a file is not in the database, e.g. a new file?
2. Parse the entire directory with `syn_parser`
  - If there is a failure in the parsing process, abort and bubble up the error
  - Ideally we would only fail in the known case of not handling malformed crates and code
3. Remove validity and edges of all nodes/edges leading to/from the target files.
  - Every node within the file has its `Validity` set to `false`
  - Every edge within the file has its `Validity` set to `false`
4. Add the recently parsed nodes and edges to the database
  - The `Validity` of these nodes and edges is `true`
  - `true` on `Validity` means that if the node/edge was previously present in the database, then there will be two copies - a previous state that is now invalid, and a current state which is valid.
    - If the node was not previously in the database, they are added to the database as having a `Validity` set to `true`
    - If a node was previously in the database, but is not in the set of parsed nodes within that file, then the nodes and edges were cleaned up in step 3 by setting their `Validity` to `false`, so they can be easily filtered on time series data targeting the current time.
5. Update the embeddings
  - Cases to consider:
    - Node added, needs embedding: `Validity` is `false` before, true now.
    - Node removed, does not need embedding: `Validity` true before, false after
    - Node updated, needs embedding: `Validity` true before, `Validity` true now, previously valid node is not equal to currently valid node
    - Node unchanged, does not need embedding: `Validity` true before, true now
  - We can run one query that retrieves all the data required to update the nodes by querying for currently valid nodes.

### Plan 2
1. Scan files in target directory, checking for changes in tracking hash.
  - return a list of changed files
  - (same as plan 1)
2. Parse entire directory (same as plan 1)
3. Before transform into database, filter the `ParsedCodeGraph` for only those nodes/edges which in the target files
4. Perform the same transform as in the full database transformation pipeline, using the filtered `ParsedCodeGraph`
  - New nodes are inserted as usual, which means with `embedding: null`
  - The new nodes with `embedding: null` 
5. Use cozo's `RETRACT` (see note) for any nodes in those files which still have embeddings, since they were not included in the parsed nodes.
  - Also retract all relations to/from the target files at the same time.
  - Retract nodes in database which are not in filtered `ParsedCodeGraph`
    - Same approach for edges
  - No need to retract nodes that have changed, since the time travel approach takes care of this for us.
6. Same embeddings process, which queries the database for unembedded nodes.

On atomic changes: Ensure steps 3-5 happen in the same transaction.

This is a much simpler plan, and should be just as effective.

### Logging
1. Number and names of files changed (added, retracted, updated)
2. Number of nodes changed
3. New embeddings generated

### Testing
Add tests for:
1. Adding new files
2. Removing files
3. Modifying existing files
4. Edge cases like empty files
5. Ensuring sub-nodes on removed items are retracted by comparing before/after on target files

#### Note on Validity
Taken from the cozo docs on [time travel](https://docs.cozodb.org/en/latest/tutorial.html#Time-travel)

TODO (Aug 3, 2025): Replace with a working example using our database once this has been implemented

##### Creating a relation with Validity

To add new valid node, the relation must have a field with the `Validity` type, e.g.
```cozo
:create mood {name: String, at: Validity => mood: String}
```

##### Putting a relation with Validity
Note that 'ASSERT' is a keyword in cozo, which uses the current moment and has "True" validity:
```cozo
?[name, at, mood] <- [['me', 'ASSERT', 'curious']]
:put mood {name, at => mood}
```

On `:put` vs `:update`, we cannot use `update` on a node that uses `Validity` unless they have the same exact timestamp. This is because `Validity` is a required field as part of the key, I believe, and `Validity` contains the timestamp.

##### Querying a relation's history with Validity
This can be queried like so:
```cozo
?[name, at, mood] := *mood{name, at, mood}
```

Which will return:
```cozo
 	name	at	mood
0	me	[1700569902203327, True]	curious
```

##### Querying a relation at the current moment
Note that the 'NOW' used below is a keyword in cozo, which automatically uses the current unix epoch
```cozo
?[name, time, mood] := *mood{name, at, mood @ 'NOW'},
                       time = format_timestamp(at)
```

Returns
```cozo
	name	time	mood
0	me	2023-11-21T12:31:42.203+00:00	curious
```

##### Retract a relation with Validity
Note that 'RETRACT' is a keyword in cozo which means this will be added with "False" validity at the current moment, so if we were to query the database for the current state after this moment, this would not appear in the results.
```cozo
?[name, at, mood] <- [['me', 'RETRACT', '']]
:put mood {name, at => mood}
```


