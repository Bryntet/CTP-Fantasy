#!/bin/bash

sea-orm-cli generate entity --with-serde deserialize --model-extra-attributes "serde(rename_all = \"PascalCase\")" -l -o /home/brynte/RustroverProjects/CTP-Fantasy/entity/src
