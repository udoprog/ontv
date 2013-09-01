from distutils.core import setup

VERSION = '0.7.10'


setup(
    name='ontv',
    version=VERSION,
    description="Your personal TV series manager",
    long_description="""
    ontv is a utility that helps you keep track of what you are looking at
    right now.

    It is based of data from thetvdb.com which requires an account and an api
    key to access, but contains information about most existing tv shows and
    their air date.

    My source of focus was to fast create a usable application that tracks the
    tv shows I am currently watching.
    """,
    author='John-John Tedro',
    author_email='johnjohn.tedro@gmail.com',
    url='http://github.com/udoprog/ontv',
    license='GPLv3',
    packages=[
        'ontv',
        'ontv.action'
    ],
    scripts=['bin/ontv'],
    install_requires=[
        'PyYAML>=3.10',
        'blessings>=1.5',
        'requests>=1.1.0',
        'python-dateutil>=2.1',
    ]
)
