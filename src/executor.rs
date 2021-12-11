use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Display;
use std::future::Future;
use std::mem::{self, ManuallyDrop};
use std::rc::Rc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use futures::future::LocalBoxFuture;
use futures::{pin_mut, FutureExt};
use scoped_tls::scoped_thread_local;

scoped_thread_local!(static EXECUTOR: LocalExecutor);

pub struct LocalExecutor {
    task_queue: TaskQueue,
}

impl LocalExecutor {
    pub fn new() -> Self {
        Self {
            task_queue: Default::default(),
        }
    }

    fn spawn_inner(future: impl Future<Output = ()> + 'static, name: Option<String>) {
        let task = Task {
            future: RefCell::new(future.boxed_local()),
            name,
        };
        println!("spawn: task {}", task);
        EXECUTOR.with(|slot| slot.task_queue.enqueue(Rc::new(task)));
    }

    pub fn spawn(future: impl Future<Output = ()> + 'static) {
        LocalExecutor::spawn_inner(future, None)
    }

    pub fn spawn_with_name(future: impl Future<Output = ()> + 'static, name: impl ToString) {
        LocalExecutor::spawn_inner(future, Some(name.to_string()))
    }

    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        println!("block_on: start");

        let output = EXECUTOR.set(self, || {
            let root_future = future;
            let root_woken = RefCell::new(true);

            let root_woken_for_waker = root_woken.clone();
            let waker = waker_fn(move || {
                println!("waking: task <root>");
                *root_woken_for_waker.borrow_mut() = true;
            });

            let cx = &mut Context::from_waker(&waker);

            pin_mut!(root_future);

            loop {
                // if the root future is woken, poll it
                if *root_woken.borrow() {
                    if let Poll::Ready(res) = root_future.as_mut().poll(cx) {
                        return res;
                    }
                }

                // consume tasks in queue
                while let Some(task) = self.task_queue.dequeue() {
                    let task_ = task.clone();
                    let mut waker = waker_fn(move || {
                        let task = task_.clone();
                        println!("waking: task {}", task);
                        EXECUTOR.with(|slot| slot.task_queue.enqueue(task));
                    });
                    let _ = task
                        .future
                        .borrow_mut()
                        .poll_unpin(&mut Context::from_waker(&mut waker));
                }

                // if the root future is woken, poll it
                if *root_woken.borrow() {
                    if let Poll::Ready(output) = root_future.as_mut().poll(cx) {
                        break output;
                    }
                }
            }
        });

        println!("block_on: finish");
        output
    }
}

pub struct TaskQueue {
    queue: RefCell<VecDeque<Rc<Task>>>,
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskQueue {
    fn new() -> Self {
        Self {
            queue: RefCell::new(VecDeque::new()),
        }
    }

    fn enqueue(&self, runnable: Rc<Task>) {
        println!("enqueue: task {}", runnable);
        self.queue.borrow_mut().push_back(runnable);
    }

    fn dequeue(&self) -> Option<Rc<Task>> {
        let maybe_task = self.queue.borrow_mut().pop_front();
        match &maybe_task {
            Some(task) => println!("dequeue: task {}", task),
            None => println!("dequeue: no task left"),
        }
        maybe_task
    }
}

struct Task {
    future: RefCell<LocalBoxFuture<'static, ()>>,
    name: Option<String>,
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            self.name
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("<unnamed>"),
        )
    }
}

pub fn waker_fn<F: Fn() + 'static>(f: F) -> Waker {
    let raw = Rc::into_raw(Rc::new(f)) as *const ();
    let vtable = &Helper::<F>::VTABLE;
    unsafe { Waker::from_raw(RawWaker::new(raw, vtable)) }
}

struct Helper<F>(F);

impl<F: Fn() + 'static> Helper<F> {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        Self::clone_waker,
        Self::wake,
        Self::wake_by_ref,
        Self::drop_waker,
    );

    unsafe fn clone_waker(ptr: *const ()) -> RawWaker {
        let rc = ManuallyDrop::new(Rc::from_raw(ptr as *const F));
        mem::forget(rc.clone());
        RawWaker::new(ptr, &Self::VTABLE)
    }

    unsafe fn wake(ptr: *const ()) {
        let rc = Rc::from_raw(ptr as *const F);
        (rc)();
    }

    unsafe fn wake_by_ref(ptr: *const ()) {
        let rc = ManuallyDrop::new(Rc::from_raw(ptr as *const F));
        (rc)();
    }

    unsafe fn drop_waker(ptr: *const ()) {
        drop(Rc::from_raw(ptr as *const F));
    }
}
