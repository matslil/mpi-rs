Instructions for AI
===================

Purpose of this repository
--------------------------

Provide an easy to use for robust and safe parallel application implementations.

MPI Object
----------

This library centers around MPI objects. These MPI objects has the same spirit as Smalltalk objects in the sense that calling a method means sending a message to the object.

Each MPI object has a single point for receiving messages.

The MPI object method implementation is written the same way as any function implementation. However, if a method of another MPI object is being called, a message is sent to that other object, state is then saved together with expected response, and then execution is returned to the object receivier. When expected response is received, the saved state is restored and method execution continues where it left of.

Implementation
--------------

Implementer defines methods for a special struct. Rust macros will then implement a trampoline where the method call will be translated into a message send to MPI object thread context. The provided implementation code for the method will then be executed in this context, and the return value will be provided as a reply message if there is a return value. If there is no return value, then no reply will be sent. If the method is a generator, then one reply message is sent for each yield, and a finish reply when method returns.

If MPI object method is called from code outside of MPI object space, then the message passing will be fully synchronous. I.e. the method trampoline will not return until a return value is available if the method should return one.

This means that there will be two sets of trampoline methods for each MPI object method, one set used by callers from outside of MPI objects, and one set for callers from another MPI object. Which set is used should preferably be selected using Rust macros, so the caller does not need to know which set to use.

Do not apply any limitation on how new a feature can be, i.e. if implementation is dependent on features only available in nightly builds this is ok.

Repo
----

Structure the repository according to best practice. Make the MPI object infrastructure available as a Rust library. Provide example applications showing how to use it. Provide unit testing code according to best practice.

Try to use descriptive names for types, methods, test cases, applications etc so that only a minimum amount of source code comment is needed.

