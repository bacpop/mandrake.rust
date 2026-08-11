# Mandrake Embedding

Mandrake creates two-dimensional stochastic cluster embeddings from sparse
distance inputs. This context defines the vocabulary for observing an embedding
while it is being calculated.

## Language

**Embedding operation**:
A caller-owned, incremental execution of one stochastic cluster embedding
calculation.
_Avoid_: Async task, background job

**Iteration budget**:
The maximum number of optimiser iterations that an embedding operation is asked
to perform in one advance.
_Avoid_: Frame count, batch size

**Current embedding**:
The single embedding state retained by an embedding operation after its most
recent completed iteration, including the initialized iteration-0 state before
any optimisation work.
_Avoid_: Frame, animation

**Final embedding**:
The current embedding retained by a completed embedding operation and available
for ownership transfer when that operation is consumed.
_Avoid_: Result frames

**Partial embedding**:
The current embedding transferred by consuming an incomplete embedding
operation.
_Avoid_: Final embedding

**Completed embedding operation**:
An embedding operation that has reached its configured optimisation-iteration
limit and retains its final current embedding.
_Avoid_: Consumed result

**Cancellation**:
Ending an embedding operation by dropping its caller-owned state before it
completes.
_Avoid_: Abort request

**Operation construction**:
The fallible creation of an embedding operation, including validation of its
input and optimisation configuration.
_Avoid_: First step

**Embedding input**:
The owned sparse COO distances and node weights consumed to construct an
embedding operation.
_Avoid_: Borrowed input, input slices

**Blocking embedding**:
An embedding calculation that advances an embedding operation to completion
before returning its final embedding.
_Avoid_: Animated result

**Embedding progress**:
The completed and configured maximum iteration counts and current `Eq`
convergence statistic reported after an operation advances.
_Avoid_: Worker updates

**No-op poll**:
An advance with an iteration budget of zero that performs no optimisation work
and reports the operation's existing progress.
_Avoid_: Empty frame

**Iteration limit**:
The configured fixed number of optimiser iterations at which an embedding
operation completes.
_Avoid_: Convergence threshold

## Distance Input

**Reader-based distance constructor**:
A distance constructor that reads a supported alignment or accessory format
from a caller-provided byte stream.
_Avoid_: Path loader

**Path loader**:
A native-only wrapper that opens a file, optionally applies compression
decoding, and passes its bytes to a reader-based distance constructor.
_Avoid_: Distance constructor

**Decompressed input**:
The raw format bytes supplied to a portable reader-based distance constructor;
any compression has already been decoded by its caller.
_Avoid_: Compressed file

**Embedding input**:
The owned COO vectors and node weights used by an embedding operation, without
sample labels.
_Avoid_: Sparse distances

**Sparse distances**:
The labeled COO distance value produced by a distance constructor.
_Avoid_: Embedding input

**Uniform weights**:
One equal positive node weight per sample, used when an embedding input is
constructed without caller-supplied weights.
_Avoid_: Default sampling

**Node count**:
The explicit number of samples represented by an embedding input, including
samples that may not appear in a COO index vector.
_Avoid_: Largest index
