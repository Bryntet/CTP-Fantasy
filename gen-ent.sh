#!/bin/bash

sea-orm-cli migrate refresh && sea-orm-cli generate entity -l -o /home/brynte/RustroverProjects/CTP-Fantasy/entity/src
