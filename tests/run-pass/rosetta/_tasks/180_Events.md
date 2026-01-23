# Events

**Problem:** Event is a synchronization object. An event has two states signaled and reset. A task may await for the event to enter the desired state, usually the signaled state. It is released once the state is entered. Releasing waiting tasks is called event notification. Programmatically controlled events can be set by a task into one of its states. In concurrent programming event also refers to a notification that some state has been reached through an asynchronous activity.

**Requirements:**
- internal, from another task, programmatically;
- external, from the hardware, such as user input, timer, etc. Signaling an event from the hardware is accomplished by means of hardware interrupts.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
