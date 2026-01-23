# Read a configuration file

**Problem:** The task is to read a configuration file in standard configuration file format, and set variables accordingly. For this task, we have a configuration file as follows: FULLNAME Foo Barber FAVOURITEFRUIT banana NEEDSPEELING ; SEEDSREMOVED OTHERFAMILY Rhu Barber, Harry Barber For the task we need to set four variables according to the configuration entries as follows: We also have an option that contains multiple parameters. These may be stored in an array. Related tasks

**Requirements:**
- This is a configuration file in standard configuration file format
- Lines beginning with a hash or a semicolon are ignored by the application
- program. Blank lines are also ignored by the application program.
- This is the fullname parameter
- This is a favourite fruit
- This boolean is commented out
- Configuration option names are not case sensitive, but configuration parameter
- data is case sensitive and may be preserved by the application program.
- An optional equals sign can be used to separate configuration parameter data
- from the option name. This is dropped by the parser.

**Success Criteria:**
- This is a boolean that should be set
