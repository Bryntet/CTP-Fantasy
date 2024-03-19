#!/bin/bash

sea-orm-cli migrate fresh && sea-orm-cli generate entity -l -o /home/brynte/RustroverProjects/CTP-Fantasy/entity/src
