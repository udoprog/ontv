def action(ns):
    print ns.term.bold_magenta(u"Compacting local databases")
    print u""

    for name, db in ns.databases.items():
        print ns.term.cyan(u"Compacting database: {0}".format(name))
        stats = db.compact()
        print ns.term.cyan(u"  pruned {0.noops} non-operation entries".format(
            stats))

    return 0


def setup(parser):
    parser.set_defaults(action=action)
