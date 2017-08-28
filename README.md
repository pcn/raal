# raal 

## A bit about working in AWS
This is a tool that helps me access nodes in AWS the way I find most useful.

Specifically I work with multiple environments (e.g. "production", "staging", "test")
in multiple regions (e.g. "us-east-1", "us-east-2", "us-west-2") using multiple accounts
which may span more than one environment and region.

Our tiers of compute resources are all named with tags that reflect the purpose of the node 
being requested.  E.g. "webnode-1-production" or "saltmaster-3-test".

## How other tools work

The other tools I've used in the past tend to ask for a 2-step process to access a system:

First, provide an exact string or a substring to match, and once
that's done, present a user with a list of matches so that they can select a match.

## Caveats with those  tools

This is very handy when the number of users and nodes are relatively
small (say, something like <50 in any environment+account+region).
However since I'm in the systems/ops/sre group, I find that I spend a
lot of my day accessing multiple systems, and the step of picking
possibly one of hundreds of systems ends up being failure-prone.
E.g. if your system enumerates systems and you meant to access system
10, but enter 19, you are now in a different system.  

What's worse is that you can't develop a muscle memory to access the systems you want.

While at first it seems obvious that you can simply access all of your
web nodes and if you want web-4-production, you can always use your enumerating lookup
tool to get to it by simply entering

    $ enumtool web-4
	
The tools I've seen that accomplish this will then make life easy if
there is only one match, and send you directly to this system, which
is a great time-saver.

As systems grow, there are three problems that I've encountered that I think
can best be handled with a slightly different style of tool:

1. You start with a tier of nodes named e.g. `web-1-test`,
   `web-2-test` etc.  Then before too long, another tier is developed that 
   names nodes e.g. `supportweb-2-test` and `authweb-2-test` and now your 
   muscle memory calls `enumtool web-2` and you now have 3 nodes that you have to 
   choose from.  You now have to choose each time because of substring matches.

2. The list of nodes can start to scroll off the screen, and then the
   tool relies on your eyes and attention span to do the filtering for
   the hosts you actually want.
   
3. As more nodes are launched and removed, you may lose the ability to
   target just the host you're interested in because e.g. `web-2-test`
   has been replaced with `web-10-test` and `web-14-test` after a
   series of launches and terminations.  You have to seearch for these new
   nodes in order to connect to them.
   

## How this works

The different style of tool that I find makes me more productive does the following
related to the above points:

1. Instead of providing a substring to be matched, allow for a regular
   expression. If you have regular expressions you can create aliases
   that access a particular system even when there are tiers with names
   that have common substrinngs.  E.g. `^web` won't match `supportweb-2-test`.
   
2. Instead of defaulting to presenting the user with a list of all
   possible matches, select one node at random from the matches, and
   just send the user there.  So if I just want a web node, I can try
   to connect to `^web`, and I'll be randomly connected to 1 of them.
   
   
3. As more nodes are launched or terminated, if there's a specific
   node I want to connect to, I can refine the regular expression,
   e.g. `^web-4` will connect me to that node without my having to
   manually select it from a list (as long as it hasn't been
   terminated.  If it has been, then just removing the `4` will give
   me another node without my having to do any manual selection).
   
There is another part of this that is subtle, but becomes a scaling
issue.  As you begin to have hundreds of nodes in an account+region,
the amount of data that needs to be transferred on each invocation of
the command can start to be pretty big.  For instance, in a production
environment:

```
|P|spacey@masonjar:~$ aws ec2 describe-instances | wc -c
2331082
```

That's counting a lot of whitespace, but even with most whitespace
eliminted/minimized, that's a fair amount of data.  So some caching
would help tremendously.

Also, it's nice to have a cache when amazon is melting down.

## Adding some flexibility in the future.

This idea usually works pretty well with ec2 instances.  The most
useful way that this can interact with ec2 is to allow you to ssh to a
running instance.  However you may want to ssh to one of a tier, or
you may want to nc to a port to see if it's open, or run mysql to
connect to an RDS, etc.  That means that this tool and its cache,
which works with ec2 instances currently should also be able to work
to get other resources; to cache them, and to give you access to them
in a sensible way (e.g. for RDS, should check to see if it's a mysql
or postgres instance and let you tell it what tool to invoke to talk
to the DB).

For right now, the flexibility is limited.  It includes:

1. Caching ec2 instance info separately per-account
2. Providing different styles of printing out data (e.g. dump json of
   instances, just print all IP addresses, just print one IP address
   randomly).
