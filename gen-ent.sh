#!/bin/bash

sea-orm-cli migrate fresh && sea-orm-cli generate entity -l --with-serde deserialize --model-extra-attributes "serde(rename_all = \"PascalCase\")" -o /home/brynte/RustroverProjects/CTP-Fantasy/entity/src
