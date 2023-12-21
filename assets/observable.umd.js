(function (global, factory) {
  typeof exports === 'object' && typeof module !== 'undefined' ? factory(exports) :
  typeof define === 'function' && define.amd ? define(['exports'], factory) :
  (global = typeof globalThis !== 'undefined' ? globalThis : global || self, factory(global.Observable = {}));
}(this, (function (exports) { 'use strict';

  class Subscription {
      constructor(observer, subscriber) {
          this.observer = observer;
          this.subscriber = subscriber;
          this.closed = false;
          observer.start(this);
          if (!this.closed) {
              this.cleanup = this.subscriber(observer);
          }
      }
      unsubscribe() {
          if (!this.closed && this.cleanup !== undefined) {
              this.cleanup();
          }
          this.closed = true;
      }
  }

  class Observable {
      constructor(subscriber) {
          this.subscriber = subscriber;
      }
      /**
       * Consolidates all the provided observables into one. The returned observable
       * will start receiving next calls once all the observable have provided at
       * least one value and will receive next calls on each new events.
       * Values are provided into an array containing all values.
       * complete is called once all the provided observables are completed.
       * error is called on each provided observable error.
       * example:
       * ```typescript
       *   const observable1 = new Subject<number>();
       *   const observable2 = new Subject<string>();
       *   Observable.all([observable1, observable2]).subscribe({
       *     next: values => console.log(values),
       *     complete: () => console.log('completed'),
       *   });
       *   observable1.next(12); // nothing happens
       *   observable2.next('yolo'); // [12, 'yolo']
       *   observable2.complete(); // nothing happens
       *   observable1.next(42); // [42, 'yolo']
       *   observable1.complete; // 'completed'
       * ```
       */
      static all(observables) {
          const values = Array.from(Array(observables.length));
          const completionState = Array.from(Array(observables.length)).map(_ => false);
          return new Observable(observer => {
              function next() {
                  if (values.every(v => v !== undefined)) {
                      observer.next !== undefined && observer.next(values);
                  }
              }
              function complete() {
                  if (completionState.every(c => c)) {
                      observer.complete !== undefined && observer.complete();
                  }
              }
              observables.forEach((observable, index) => {
                  observable.subscribe({
                      next: (value) => {
                          values[index] = value;
                          next();
                      },
                      complete: () => {
                          completionState[index] = true;
                          complete();
                      },
                      error: observer.error,
                  });
              });
              return () => { };
          });
      }
      static complete(value) {
          return new Observable((observer) => {
              if (value !== undefined) {
                  observer.next(value);
              }
              observer.complete();
              return () => { };
          });
      }
      /**
       * Chains observable in order to smoothly apply processing on next values.
       * Error and complete signals are being forwarded to returned observable.
       * Errors can be raised to the error parameter of the next function parameter.
       * example:
       * ```typescript
       * const observable = new Observable<number>(observer => {
       *   observer.next(42);
       *   return () => {};
       * });
       *
       * let observedValue: string;
       * Observable.chain(observable, value => value.toString()).subscribe({
       *   next: value => observedValue = value,
       * });
       * ```
       */
      static chain(observable, next) {
          return new Observable(observer => {
              observable.subscribe({
                  next: value => observer.next(next(value, observer.error)),
                  error: observer.error,
                  complete: observer.complete,
              });
              return () => { };
          });
      }
      // Returns itself
      observable() {
          return this;
      }
      // Converts items to an Observable
      static of(...items) {
          return new Observable(observer => {
              items.forEach(i => observer.next(i));
              observer.complete();
              return () => { };
          });
      }
      // Converts an observable or iterable to an Observable
      static from(convertee) {
          if (convertee.hasOwnProperty('observable')) {
              const observable = convertee;
              return observable.observable();
          }
          const iterable = convertee;
          return new Observable(observer => {
              for (const value of iterable) {
                  observer.next(value);
              }
              observer.complete();
              return () => { };
          });
      }
      subscribe(observer, error = () => { }, complete = () => { }) {
          if (typeof observer === 'function') {
              return new Subscription({
                  start: (_subscription) => { },
                  next: observer,
                  error,
                  complete,
              }, this.subscriber);
          }
          else {
              return new Subscription({
                  start: observer.start !== undefined ? observer.start : () => { },
                  next: observer.next !== undefined ? observer.next : () => { },
                  error: observer.error !== undefined ? observer.error : () => { },
                  complete: observer.complete !== undefined ? observer.complete : () => { },
              }, this.subscriber);
          }
      }
  }

  class Subject extends Observable {
      constructor() {
          super((observer) => {
              this.observers.push(observer);
              return () => {
                  this.observers = this.observers.filter((o) => o !== observer);
              };
          });
          this.observers = [];
      }
      next(value) {
          // Broadcast to all observers
          this.observers.forEach(observer => observer.next(value));
      }
      error(errValue) {
          this.observers.forEach(observer => observer.error(errValue));
      }
      complete() {
          this.observers.forEach(observer => observer.complete());
      }
  }

  /**
   * Always hold one value, which can be recalled at any time with get.
   * Basically a variable with subscription.
   */
  class BehaviorSubject extends Subject {
      /**
       * Construct a Subject by providing an intial value which will be immediatly
       * forwarded to the subscribed Observers.
       */
      constructor(value) {
          super();
          this.value = value;
          this.errValue = undefined;
          this.last = value;
          this.next(value);
      }
      /**
       * Create a BehaviorSubject from a Subject.
       * The call is asynchronous and resolves once the first value is received.
       * Subsequent values are then forwarded.
       */
      static fromSubject(subject) {
          return new Promise(resolve => {
              let initialized = false;
              const subscription = subject.subscribe(value => {
                  if (!initialized) {
                      initialized = true;
                      const behaviorSubject = new BehaviorSubject(value);
                      subject.subscribe({
                          next: value => behaviorSubject.next(value),
                          error: error => behaviorSubject.error(error),
                          complete: () => behaviorSubject.complete(),
                      });
                      // We can only unsubscribe asynchonously when we are subscribing.
                      setTimeout(() => {
                          subscription.unsubscribe();
                          resolve(behaviorSubject);
                      }, 0);
                  }
              });
          });
      }
      error(errValue) {
          this.errValue = errValue;
          super.error(errValue);
      }
      next(value) {
          this.last = value;
          super.next(value);
      }
      /**
       * Retrieve the last set value
       */
      get() {
          if (this.hasError()) {
              throw this.errValue;
          }
          return this.last;
      }
      /**
       * Notifies subscribing observer immediatly.
       */
      subscribe(observer, error = () => { }, complete = () => { }) {
          const subscription = super.subscribe(observer, error, complete);
          subscription.observer.next(this.last);
          return subscription;
      }
      hasError() {
          return this.errValue !== undefined;
      }
  }

  /**
   * Will replay the previous calls to next, complete and error to a newly
   * subscribe subject.
   */
  class ReplaySubject extends Observable {
      // memorySize: number of event saved and replayed on subscription
      constructor(memorySize) {
          super((observer) => {
              this.observers.push(observer);
              this.memory.forEach((value) => observer.next(value));
              if (this.hasError()) {
                  observer.error(this.errValue);
              }
              else if (this.isComplete()) {
                  observer.complete();
              }
              return () => {
                  this.observers = this.observers.filter((o) => o !== observer);
              };
          });
          this.memorySize = memorySize;
          this.observers = [];
          this.memory = [];
          this.errValue = undefined;
          this.completed = false;
      }
      next(value) {
          // Save the value for later replay on subscription
          this.remember(value);
          // Broadcast to all observers
          this.observers.forEach(observer => observer.next(value));
      }
      error(errValue) {
          this.errValue = errValue;
          this.observers.forEach(observer => observer.error(errValue));
      }
      complete() {
          this.completed = true;
          this.observers.forEach(observer => observer.complete());
      }
      hasError() {
          return this.errValue !== undefined;
      }
      isComplete() {
          return this.completed;
      }
      remember(value) {
          if (this.memorySize > 0) {
              if (this.memory.length >= this.memorySize) {
                  const [_head, ...tail] = this.memory;
                  this.memory = tail;
              }
              this.memory.push(value);
          }
      }
  }

  /**
   * This Subject does not broadcast the value immediatly but only when the
   * browser is idle using the requestIdleCallback facility.
   * Observers should not expect to receive all updates but only the last one
   * received before the browser became idle.
   */
  class IdleSubject extends ReplaySubject {
      /**
       * window: the brower's window object or equivalent
       * timeout: optional parameter provided to requestIdCallback
       */
      constructor(window, timeout) {
          super(0);
          this.window = window;
          this.timeout = timeout;
          this.idleHandle = 0;
      }
      next(value) {
          if (this.idleHandle !== 0) {
              this.window.cancelIdleCallback(this.idleHandle);
          }
          this.idleHandle = this.window.requestIdleCallback(() => {
              // Broadcast to all observers
              this.observers.forEach((observer) => observer.next(value), {
                  timeout: this.timeout,
              });
          });
          // Save the value for later replay on subscription
          this.remember(value);
      }
  }

  exports.BehaviorSubject = BehaviorSubject;
  exports.IdleSubject = IdleSubject;
  exports.Observable = Observable;
  exports.ReplaySubject = ReplaySubject;
  exports.Subject = Subject;
  exports.Subscription = Subscription;

  Object.defineProperty(exports, '__esModule', { value: true });

})));
//# sourceMappingURL=observable.umd.js.map
