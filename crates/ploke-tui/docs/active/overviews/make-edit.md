# Overview of LLM making edit and user approving

1. LLM submits edit proposal through too call using `GatCodeEdit` with params
  - file: file path for edits
  - canon: canonical path to target of edit
  - node_type: type of node edited
  - code: the edited content used to replace original node content

todo:
  - add larger description to "node_type" field of `GatCodeEdit`
