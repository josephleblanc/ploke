---
name: run-protocol
description: Use before and after formal eval runs so configuration, validity guards, telemetry capture, and result recording are handled consistently.
---

# Run Protocol

Use this skill for formal runs that will enter the record.

## Pre-Run

- freeze the subset
- identify the hypothesis and EDR
- commit the config version
- check provider and setup health
- confirm validity guards are acceptable

## Run

- capture full telemetry
- preserve artifact paths
- fail gracefully with typed status if the run cannot continue

## Post-Run

- compute or collect metrics
- classify failures
- update the EDR and evidence ledger
- add a lab-book note if the result changed the plan
